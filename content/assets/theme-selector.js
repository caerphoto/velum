(function (D) {
    const selector = D.querySelector('#theme-selector');
    const styleTag = D.querySelector('#main-style-tag');
    const currentTheme = styleTag.getAttribute('href');

    const TWO_YEARS = 60*60*24*365*2;

    function changeTheme() {
        styleTag.setAttribute('href', selector.value);
        D.cookie = `theme=${selector.value}; path=/; max-age=${TWO_YEARS}`;
    }

    selector.addEventListener('change', changeTheme);
    if (currentTheme) {
        const option = selector.querySelector(`[value="${currentTheme}"]`)
        if (option) { option.selected = true; }
    }

}(window.document));
