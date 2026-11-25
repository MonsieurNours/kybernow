# libkybernow_pyrowave.so : PyroWave (chemin Vulkan, sources au commit épinglé
# dans third_party/pyrowave, Granite partiel) et le pont de l'app
# (kn_pyrowave.cpp : device, décodeur, swapchain). Construite par
# apps/android/scripts/build-native.sh avec la chaîne CMake du NDK ;
# libkybernow.so s'y lie (fonctionnalité Cargo `pyrowave` de kybernow-android).
cmake_minimum_required(VERSION 3.20)
project(kybernow_pyrowave LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(PYROWAVE_DIR "" CACHE PATH "Sources de PyroWave (third_party/pyrowave)")
if (NOT EXISTS "${PYROWAVE_DIR}/pyrowave.h")
    message(FATAL_ERROR "PyroWave introuvable : PYROWAVE_DIR=${PYROWAVE_DIR}")
endif()
find_program(GLSLC glslc REQUIRED)

# Granite réduit à ce qu'il faut au codec (comme la bibliothèque partagée de
# l'amont) : ni rendu, ni compilateur de shaders, ni fenêtres.
set(GRANITE_VULKAN_SHADER_MANAGER_RUNTIME_COMPILER OFF CACHE BOOL "" FORCE)
set(GRANITE_SHADER_COMPILER_OPTIMIZE OFF CACHE BOOL "" FORCE)
set(GRANITE_POSITION_INDEPENDENT ON CACHE BOOL "" FORCE)
set(GRANITE_SHIPPING ON CACHE BOOL "" FORCE)
set(GRANITE_HIDDEN ON CACHE BOOL "" FORCE)
set(GRANITE_VULKAN_SPIRV_CROSS OFF CACHE BOOL "" FORCE)
set(GRANITE_VULKAN_SYSTEM_HANDLES OFF CACHE BOOL "" FORCE)
set(GRANITE_RENDERER OFF CACHE BOOL "" FORCE)
set(GRANITE_VULKAN_FOSSILIZE OFF CACHE BOOL "" FORCE)
set(GRANITE_RENDERDOC_CAPTURE OFF CACHE BOOL "" FORCE)
set(GRANITE_FFMPEG OFF CACHE BOOL "" FORCE)
set(GRANITE_FFMPEG_VULKAN OFF CACHE BOOL "" FORCE)
set(GRANITE_TOOLS OFF CACHE BOOL "" FORCE)
set(GRANITE_INSTALL_TARGETS OFF CACHE BOOL "" FORCE)
set(GRANITE_ANDROID_APK_FILESYSTEM OFF CACHE BOOL "" FORCE)
set(GRANITE_PLATFORM "null" CACHE STRING "" FORCE)
add_subdirectory(${PYROWAVE_DIR}/Granite granite EXCLUDE_FROM_ALL)
add_subdirectory(${PYROWAVE_DIR} pyrowave EXCLUDE_FROM_ALL)

# Shaders de l'affichage (GLSL → SPIR-V par le glslc du NDK), inclus comme tableaux.
set(SHADER_OUT ${CMAKE_CURRENT_BINARY_DIR}/shaders)
file(MAKE_DIRECTORY ${SHADER_OUT})
foreach(stage vert frag)
    add_custom_command(
        OUTPUT ${SHADER_OUT}/present.${stage}.inc
        COMMAND ${GLSLC} --target-env=vulkan1.3 -O -mfmt=num -o ${SHADER_OUT}/present.${stage}.inc
                ${CMAKE_CURRENT_SOURCE_DIR}/shaders/present.${stage}
        DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/shaders/present.${stage}
        VERBATIM)
endforeach()

# Le pont et l'API C de PyroWave, compilés une fois pour la bibliothèque et
# pour l'essai hors ligne.
add_library(kybernow_pyrowave_objects OBJECT
        kn_pyrowave.cpp
        ${PYROWAVE_DIR}/pyrowave_c.cpp
        ${PYROWAVE_DIR}/Granite/video/scaler.cpp
        ${SHADER_OUT}/present.vert.inc
        ${SHADER_OUT}/present.frag.inc)
target_include_directories(kybernow_pyrowave_objects PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR} ${SHADER_OUT} ${PYROWAVE_DIR} ${PYROWAVE_DIR}/shaders ${PYROWAVE_DIR}/Granite/video)
target_link_libraries(kybernow_pyrowave_objects PUBLIC pyrowave granite-vulkan granite-math granite-util)
target_compile_options(kybernow_pyrowave_objects PRIVATE -fvisibility=hidden -Wall -Wno-missing-field-initializers)
target_compile_definitions(kybernow_pyrowave_objects PRIVATE KN_PYROWAVE_BUILD)

add_library(kybernow_pyrowave SHARED $<TARGET_OBJECTS:kybernow_pyrowave_objects>)
target_link_libraries(kybernow_pyrowave PRIVATE kybernow_pyrowave_objects android log)
# Seules les fonctions kn_pw_* sont exportées ; runtime C++ lié en statique
# (ANDROID_STL=c++_static), rien d'autre que Bionic, liblog et libandroid.
target_link_options(kybernow_pyrowave PRIVATE -Wl,--exclude-libs,ALL -Wl,--no-undefined -Wl,-z,max-page-size=16384
        "-Wl,--version-script=${CMAKE_CURRENT_SOURCE_DIR}/exports.map")
set_target_properties(kybernow_pyrowave PROPERTIES LINK_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/exports.map)

# Essai hors ligne (apps/android/tests/pyrowave/run.sh) : sonde, encodage par
# l'encodeur de l'amont, décodage et présentation sur une surface AImageReader.
add_executable(kn_pyrowave_test tests/kn_pyrowave_test.cpp $<TARGET_OBJECTS:kybernow_pyrowave_objects>)
target_link_libraries(kn_pyrowave_test PRIVATE kybernow_pyrowave_objects android log mediandk)
target_compile_definitions(kn_pyrowave_test PRIVATE KN_PYROWAVE_BUILD)
target_link_options(kn_pyrowave_test PRIVATE -Wl,-z,max-page-size=16384)
