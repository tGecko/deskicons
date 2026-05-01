cmake_minimum_required(VERSION 3.20)

project(deskicons LANGUAGES CXX RC)

add_executable(deskicons
    src/main.cpp
    src/deskicons.rc
    src/app.manifest
)

target_compile_features(deskicons PRIVATE cxx_std_20)
target_compile_definitions(deskicons PRIVATE UNICODE _UNICODE NOMINMAX WIN32_LEAN_AND_MEAN)
set_target_properties(deskicons PROPERTIES WIN32_EXECUTABLE TRUE)

if (MSVC)
    target_link_options(deskicons PRIVATE /ENTRY:mainCRTStartup)
endif()

if (MSVC)
    target_compile_options(deskicons PRIVATE /W4 /permissive- /EHsc)
    # Replace CMake's default /O2 (speed) with /O1 (size) for Release.
    string(REPLACE "/O2" "/O1" CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE}")
    string(REPLACE "/Ob2" "/Ob1" CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE}")
    # /Os: favor small code; /Gy: function-level linking (dead-strip with /OPT:REF);
    # /GF: read-only string pooling; /GL: whole-program optimization.
    target_compile_options(deskicons PRIVATE
        $<$<CONFIG:Release>:/Os /Gy /GF /GL>
    )
    target_link_options(deskicons PRIVATE
        $<$<CONFIG:Release>:/LTCG /OPT:REF /OPT:ICF>
    )
else()
    target_compile_options(deskicons PRIVATE -Wall -Wextra -Wpedantic)
    target_compile_options(deskicons PRIVATE $<$<CONFIG:Release>:-Os>)
endif()

target_link_libraries(deskicons PRIVATE
    ole32
    oleaut32
    advapi32
    comctl32
    gdiplus
    shell32
    shlwapi
    user32
    uuid
)

install(TARGETS deskicons RUNTIME DESTINATION .)
install(FILES README.dist.md DESTINATION . RENAME README.md)
install(FILES README_de.dist.md DESTINATION . RENAME README_de.md)

set(CPACK_PACKAGE_NAME "DeskIcons")
set(CPACK_PACKAGE_VENDOR "DeskIcons")
set(CPACK_PACKAGE_VERSION "0.1.7")
set(CPACK_GENERATOR "ZIP")
set(CPACK_PACKAGE_FILE_NAME "DeskIcons-0.1.7-win64")
include(CPack)
