(function (W, D) {
    const selector = D.querySelector('#theme-selector');
    const styleTag = D.querySelector('#main-style-tag');
    function changeTheme() {
        styleTag.setAttribute('href', selector.value);
    }

    selector.addEventListener('change', changeTheme);
}(window, window.document));
