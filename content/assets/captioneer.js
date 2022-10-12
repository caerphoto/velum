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

    function appendClass(element, className) {
        element.className += ' ' + className;
    }

    var images = Array.prototype.slice.call(D.querySelectorAll('p > img'));
    var loneImages = images.filter(function (image) {
        return image.parentNode.children.length === 1;
    });

    getParents(images).forEach(function (element) {
        appendClass(element, 'img');
    });

    loneImages.forEach(function (image) {
        var figure = D.createElement('figure');
        var caption = D.createElement('figcaption');

        var parent = image.parentNode;
        var text = image.getAttribute('alt');

        image.setAttribute('data-action', 'zoom');

        caption.appendChild(D.createTextNode(text));

        figure.appendChild(image);
        figure.appendChild(caption);
        parent.appendChild(figure);
    });

}(document));
