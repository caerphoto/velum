(function (D) {
    const selector = D.querySelector('#theme-selector');
    const styleTag = D.querySelector('#main-style-tag');

    if (!selector) return;

    const currentTheme = styleTag.getAttribute('href');

    const TEN_YEARS = 60*60*24*365*2;

    function changeTheme() {
        // Append timestamp to query string to prvent cache. When page is next
        // loaded, the proper timestamped version will be used.
        const themeUrl = `/assets/themes/${selector.value}?_=${Date.now()}`;
        styleTag.setAttribute('href', themeUrl);
        D.cookie = `theme=${selector.value}; path=/; max-age=${TEN_YEARS}`;
    }

    selector.addEventListener('change', changeTheme);
    if (currentTheme) {
        const option = selector.querySelector(`[value="${currentTheme}"]`)
        if (option) { option.selected = true; }
    }

}(window.document));
