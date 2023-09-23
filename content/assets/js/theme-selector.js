(function(D) {
    const selector = D.querySelector('#theme-selector-box');
    if (!selector) return;

    const root = D.documentElement;
    const validThemes = Array.from(selector.querySelectorAll('input'))
        .map(i => i.value)
        .reduce((obj, val) => { obj[val] = true; return obj }, {});
    const DEFAULT_THEME = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';

    function getCookie(name) {
        const value = `; ${document.cookie}`;
        const parts = value.split(`; ${name}=`);
        if (parts.length === 2) return parts.pop().split(';').shift();
    }

    let currentTheme = getCookie('theme') || DEFAULT_THEME;
    if (!validThemes[currentTheme]) currentTheme = DEFAULT_THEME;

    const TEN_YEARS = 60 * 60 * 24 * 365 * 2;

    function changeTheme(event) {
        const theme = event.target.value;
        root.className = theme;
        D.cookie = `theme=${theme}; path=/; max-age=${TEN_YEARS}; SameSite=strict`;
    }

    selector.addEventListener('change', changeTheme);
    if (currentTheme) {
        const option = selector.querySelector(`[value="${currentTheme}"]`)
        if (option) {
            option.setAttribute("checked", "true");
            changeTheme({ target: option });
        }
    }

}(window.document));
