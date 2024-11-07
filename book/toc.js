// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
var sidebarScrollbox = document.querySelector("#sidebar .sidebar-scrollbox");
sidebarScrollbox.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><a href="introduction.html"><strong aria-hidden="true">1.</strong> Introduction</a></li><li class="chapter-item expanded "><a href="tutorial/introduction.html"><strong aria-hidden="true">2.</strong> Tutorial</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="tutorial/preparation.html"><strong aria-hidden="true">2.1.</strong> Preparation</a></li><li class="chapter-item expanded "><a href="tutorial/finding_gir_files.html"><strong aria-hidden="true">2.2.</strong> Finding .gir files</a></li><li class="chapter-item expanded "><a href="tutorial/sys_library.html"><strong aria-hidden="true">2.3.</strong> Generating the FFI library</a></li><li class="chapter-item expanded "><a href="tutorial/high_level_rust_api.html"><strong aria-hidden="true">2.4.</strong> Generating the Rust API</a></li><li class="chapter-item expanded "><a href="tutorial/handling_errors.html"><strong aria-hidden="true">2.5.</strong> Handling generation errors</a></li><li class="chapter-item expanded "><a href="tutorial/generate_docs.html"><strong aria-hidden="true">2.6.</strong> Generating documentation</a></li></ol></li><li class="chapter-item expanded "><a href="config/introduction.html"><strong aria-hidden="true">3.</strong> Configuration files</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="config/ffi.html"><strong aria-hidden="true">3.1.</strong> FFI Options</a></li><li class="chapter-item expanded "><a href="config/api.html"><strong aria-hidden="true">3.2.</strong> API Options</a></li><li class="chapter-item expanded "><a href="config/name_override.html"><strong aria-hidden="true">3.3.</strong> Crate name override</a></li></ol></li></ol>';
(function() {
    let current_page = document.location.href.toString();
    if (current_page.endsWith("/")) {
        current_page += "index.html";
    }
    var links = sidebarScrollbox.querySelectorAll("a");
    var l = links.length;
    for (var i = 0; i < l; ++i) {
        var link = links[i];
        var href = link.getAttribute("href");
        if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
            link.href = path_to_root + href;
        }
        // The "index" page is supposed to alias the first chapter in the book.
        if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
            link.classList.add("active");
            var parent = link.parentElement;
            while (parent) {
                if (parent.tagName === "LI" && parent.previousElementSibling) {
                    if (parent.previousElementSibling.classList.contains("chapter-item")) {
                        parent.previousElementSibling.classList.add("expanded");
                    }
                }
                parent = parent.parentElement;
            }
        }
    }
})();

// Track and set sidebar scroll position
sidebarScrollbox.addEventListener('click', function(e) {
    if (e.target.tagName === 'A') {
        sessionStorage.setItem('sidebar-scroll', sidebarScrollbox.scrollTop);
    }
}, { passive: true });
var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
sessionStorage.removeItem('sidebar-scroll');
if (sidebarScrollTop) {
    // preserve sidebar scroll position when navigating via links within sidebar
    sidebarScrollbox.scrollTop = sidebarScrollTop;
} else {
    // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
    var activeSection = document.querySelector('#sidebar .active');
    if (activeSection) {
        activeSection.scrollIntoView({ block: 'center' });
    }
}
