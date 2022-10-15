(function (D) {
    const selector = D.querySelector('#theme-selector');
    const styleTag = D.querySelector('#main-style-tag');

    if (!selector) return;

    function getCookie(name) {
        const value = `; ${document.cookie}`;
        const parts = value.split(`; ${name}=`);
        if (parts.length === 2) return parts.pop().split(';').shift();
    }

    // TODO: don't hard-code default selection
    const currentTheme = getCookie('theme') || 'topo.css';

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
