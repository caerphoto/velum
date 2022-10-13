(function (W, D) {
    function getIntValue(prop, el) {
        const sv = W.getComputedStyle(el).getPropertyValue(prop);
        return parseInt(sv, 10);
    }

    const output = D.querySelector('#debug');
    const fontSize = getIntValue('font-size', D.body);
    output.classList.add('active');

    function updateOutput() {
        const width = getIntValue('width', D.body);
        const ems = (width / fontSize).toPrecision(3);
        output.innerHTML = `width: ${ems}em`;
    }
    W.addEventListener('resize', updateOutput);
    updateOutput();
}(window, window.document));
