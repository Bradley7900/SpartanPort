"""Dependency resolution and redistribution boundaries of the Mac package."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from paths import ROOT

spec = importlib.util.spec_from_file_location('macos_bundle', ROOT / 'packaging/macos/bundle_libraries.py')
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


class MacosPackageTest(unittest.TestCase):
    @unittest.skipIf(sys.platform == 'win32', 'Windows requires privileges to create symbolic links')
    def test_vulkan_manifest_and_loader_aliases_are_bundle_relative(self):
        with tempfile.TemporaryDirectory() as directory:
            runtime = Path(directory)
            loader = runtime / 'lib/libvulkan.1.4.357.dylib'
            driver = runtime / 'vulkan/libvulkan_kosmickrisp.dylib'
            for file in (loader, driver):
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b'library')
            bundle.configure_vulkan(runtime)
            manifest = json.loads((driver.parent / 'kosmickrisp_mesa_icd.json').read_text())
            self.assertEqual(manifest['ICD']['library_path'], './' + driver.name)
            self.assertEqual((driver.parent / manifest['ICD']['library_path']).resolve(), driver.resolve())
            for name in ('libvulkan.dylib', 'libvulkan.1.dylib'):
                alias = loader.parent / name
                self.assertTrue(alias.is_symlink())
                self.assertEqual(alias.readlink(), Path(loader.name))
                self.assertEqual(alias.resolve(), loader.resolve())

    def test_missing_vulkan_libraries_stop_packaging(self):
        for missing in ('loader', 'driver'):
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as directory:
                runtime = Path(directory)
                for kind, name in (('loader', 'lib/libvulkan.1.4.357.dylib'),
                                   ('driver', 'vulkan/libvulkan_kosmickrisp.dylib')):
                    if kind != missing:
                        file = runtime / name
                        file.parent.mkdir(parents=True, exist_ok=True)
                        file.write_bytes(b'library')
                with self.assertRaisesRegex(ValueError, 'Vulkan'):
                    bundle.configure_vulkan(runtime)

    def test_relocates_transitive_loader_path_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prefix = root / 'deps'
            runtime = root / 'bundle/bin'
            for name in ('spartanport-probe', 'spartanport-gpu-capabilities',
                         'gpu/libspartanportgpu.dylib', 'vulkan/libvulkan_kosmickrisp.dylib'):
                file = runtime / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b'binary')
            for name in ('libSDL3.dylib', 'libhelper.dylib'):
                file = prefix / 'lib' / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(name.encode())

            def tool(*command):
                if command[0] == 'lipo':
                    return 'x86_64\n'
                file = Path(command[-1])
                if command[1] == '-L':
                    dependency = (str(prefix / 'lib/libSDL3.dylib') if file.name == 'spartanport-probe' else
                                  '@loader_path/libhelper.dylib' if file.name == 'libSDL3.dylib' else
                                  '/usr/lib/libSystem.B.dylib')
                    return f'{file}:\n\t{dependency} (compatibility version 1.0.0)\n'
                return f'{file}:\n'

            with mock.patch.object(bundle, 'output', side_effect=tool), mock.patch.object(bundle.subprocess, 'run') as run:
                bundle.relocate(runtime, prefix)
            self.assertEqual((runtime / 'lib/libhelper.dylib').read_bytes(), b'libhelper.dylib')
            self.assertIn(mock.call(['install_name_tool', '-change', '@loader_path/libhelper.dylib',
                                    '@rpath/libhelper.dylib', str(runtime / 'lib/libSDL3.dylib')], check=True), run.call_args_list)

    def test_arm_only_library_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(bundle, 'output', return_value='arm64\n'):
            with self.assertRaisesRegex(ValueError, 'not x86-64'):
                bundle.relocate(Path(directory), Path(directory))

    def test_resolves_rpath_and_absolute_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory)
            library = prefix / 'lib/libSDL3.dylib'
            library.parent.mkdir()
            library.write_bytes(b'library')
            self.assertEqual(bundle.resolve_library('@rpath/libSDL3.dylib', prefix / 'probe', prefix), library.resolve())
            self.assertEqual(bundle.resolve_library(str(library), prefix / 'probe', prefix), library.resolve())
            self.assertIsNone(bundle.resolve_library('/usr/lib/libSystem.B.dylib', prefix / 'probe', prefix))

    def test_missing_or_external_dependency_stops_packaging(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory) / 'deps'
            prefix.mkdir()
            external = prefix.parent / 'developer-only.dylib'
            external.write_bytes(b'library')
            for name in ('@rpath/missing.dylib', str(external)):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    bundle.resolve_library(name, prefix / 'probe', prefix)

    def test_package_rejects_game_and_user_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'LICENSE').write_text('license')
            bundle.verify_contents(root)
            for name in ('eboot.bin', 'settings.json', 'spartanport.ini', 'last_run.log', 'shader.spv'):
                file = root / name
                file.write_bytes(b'private')
                with self.subTest(name=name), self.assertRaises(ValueError):
                    bundle.verify_contents(root)
                file.unlink()


@unittest.skipUnless(sys.platform == 'darwin' and os.environ.get('SPARTAN_MACOS_PACKAGE'),
                     'Requires the mounted macOS test package')
class MacosVulkanSmokeTest(unittest.TestCase):
    def test_packaged_loader_and_driver_without_system_vulkan(self):
        runtime = Path(os.environ['SPARTAN_MACOS_PACKAGE']).resolve()
        deps = Path(os.environ['SPARTAN_MACOS_DEPS'])
        manifest = runtime / 'vulkan/kosmickrisp_mesa_icd.json'
        data = json.loads(manifest.read_text())
        self.assertEqual(data['ICD']['library_path'], './libvulkan_kosmickrisp.dylib')
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            source = work / 'vulkan-smoke.c'
            binary = work / 'vulkan-smoke'
            source.write_text(r'''
#include <dlfcn.h>
#include <mach-o/dyld.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vulkan/vulkan.h>
int main(int argc, char **argv) {
    if (argc != 2) return 1;
    void *loader = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!loader) { fprintf(stderr, "%s\n", dlerror()); return 2; }
    PFN_vkGetInstanceProcAddr get = (PFN_vkGetInstanceProcAddr)dlsym(loader, "vkGetInstanceProcAddr");
    if (!get) return 3;
    PFN_vkEnumerateInstanceExtensionProperties enumerate =
        (PFN_vkEnumerateInstanceExtensionProperties)get(NULL, "vkEnumerateInstanceExtensionProperties");
    if (!enumerate) return 4;
    uint32_t count = 0;
    if (enumerate(NULL, &count, NULL) != VK_SUCCESS || !count) return 5;
    VkExtensionProperties *extensions = calloc(count, sizeof(*extensions));
    if (!extensions) return 6;
    if (enumerate(NULL, &count, extensions) != VK_SUCCESS) return 7;
    int surface = 0, metal = 0;
    for (uint32_t i = 0; i < count; ++i) {
        surface |= !strcmp(extensions[i].extensionName, "VK_KHR_surface");
        metal |= !strcmp(extensions[i].extensionName, "VK_EXT_metal_surface");
    }
    free(extensions);
    if (!surface || !metal) return 8;
    PFN_vkCreateInstance create = (PFN_vkCreateInstance)get(NULL, "vkCreateInstance");
    if (!create) return 9;
    const char *enabled[] = {"VK_KHR_surface", "VK_EXT_metal_surface", "VK_KHR_portability_enumeration"};
    const VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
                                   .apiVersion = VK_API_VERSION_1_3};
    const VkInstanceCreateInfo info = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        .flags = VK_INSTANCE_CREATE_ENUMERATE_PORTABILITY_BIT_KHR, .pApplicationInfo = &app,
        .enabledExtensionCount = 3, .ppEnabledExtensionNames = enabled};
    VkInstance instance = VK_NULL_HANDLE;
    VkResult result = create(&info, NULL, &instance);
    if (result != VK_SUCCESS) { fprintf(stderr, "vkCreateInstance: %d\n", result); return 10; }
    for (uint32_t i = 0; i < _dyld_image_count(); ++i)
        printf("IMAGE %s\n", _dyld_get_image_name(i));
    puts("Bundled Vulkan instance created with Metal surface extensions");
    PFN_vkDestroyInstance destroy = (PFN_vkDestroyInstance)get(instance, "vkDestroyInstance");
    if (!destroy) return 11;
    destroy(instance, NULL);
    dlclose(loader);
    return 0;
}
''', encoding='utf-8')
            subprocess.run(['clang', '-arch', 'x86_64', '-Wall', '-Wextra', '-Werror',
                            '-I' + str(deps / 'include'), str(source),
                            '-Wl,-rpath,' + str(runtime / 'lib'), '-o', str(binary)],
                           check=True, timeout=60)
            env = {key: value for key, value in os.environ.items()
                   if not key.startswith(('VK_', 'DYLD_', 'SPARTAN_'))}
            env.update(HOME=directory, VK_DRIVER_FILES=str(manifest),
                       VK_ICD_FILENAMES=str(manifest), ROSETTA_ADVERTISE_AVX='1')
            for name in ('libvulkan.dylib', 'libvulkan.1.dylib'):
                with self.subTest(loader=name):
                    alias = runtime / 'lib' / name
                    self.assertTrue(alias.is_symlink())
                    self.assertTrue(alias.resolve().is_relative_to(runtime))
                    result = subprocess.run(['arch', '-x86_64', str(binary), name],
                                            env=env, cwd=directory, capture_output=True,
                                            text=True, timeout=60)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    images = [Path(line.removeprefix('IMAGE ')).resolve()
                              for line in result.stdout.splitlines() if line.startswith('IMAGE ')]
                    self.assertIn(alias.resolve(), images)
                    self.assertIn((runtime / 'vulkan/libvulkan_kosmickrisp.dylib').resolve(), images)
                    for image in images:
                        self.assertTrue(image == binary or image.is_relative_to(runtime) or
                                        str(image).startswith(('/System/', '/usr/lib/', '/Library/Apple/')),
                                        f'Package loaded an external library: {image}')
                    print(result.stdout, end='')


if __name__ == '__main__':
    unittest.main()
