/* Adds the class 'img' to paragraphs containing images. The .img class can be
 * used to, for example, make these paragraphs wider than regular text
 * paragraphs.
 * Also addes data-action="zoom" to images so they work with the Medium.com
 * image zoom thing ( https://github.com/nishanths/zoom.js ).
 * */
(function (D) {
    'use strict';

    function getParents(elements) {
        return elements.map(function (node) {
            return node.parentNode;
        });
    }

    const images = Array.from(D.querySelectorAll('p > img'));
    const loneImages = images.filter(function (image) {
        return image.parentNode.children.length === 1;
    });

    getParents(images).forEach(function (element) {
        element.classList.add('img');
    });

    loneImages.forEach(function (image) {
        const figure = D.createElement('figure');
        const caption = D.createElement('figcaption');

        const parent = image.parentNode;
        const text = image.getAttribute('alt');

        image.setAttribute('data-action', 'zoom');

        caption.appendChild(D.createTextNode(text));

        figure.appendChild(image);
        figure.appendChild(caption);
        parent.appendChild(figure);
    });

}(document));
