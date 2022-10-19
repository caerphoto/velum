/* global ROOT_ELEMENT */
// typography.js
// Replaces quote marks and hyphens with their proper typographical equivalents.
// ROOT_ELEMENT can be specified in the form of a CSS selector string to limit
// replacement to that element.

function getPreNodes(root) {
    const nodes = [];
    const nodeList = root.querySelectorAll("pre");

    for (let i = 0, l = nodeList.length; i < l; i += 1) {
        nodes.push(nodeList[i]);
    }

    return nodes;
}

function isContainedInAny(node, parents) {
    // Checks if node is a descendant of any of parents.
    return parents.some(function (el) {
        return el.contains(node);
    });
}

function skipWhitespaceOnly(node) {
    // Skip whitespace-only nodes.
    if (/^\s*$/.test(node.data)) {
        return NodeFilter.FILTER_SKIP;
    } else {
        return NodeFilter.FILTER_ACCEPT;
    }
}

function getTextNodes(root) {
    // Return an array of text nodes, except those that are inside <pre>,
    // <style> or <script> elements, or contain only whitespace.
    const nodes = [];
    let node;
    let parentNodeName;
    const walker = document.createTreeWalker(
            root,
            NodeFilter.SHOW_TEXT,
            //{ acceptNode: skipWhitespaceOnly },
            skipWhitespaceOnly,
            false
        );
    const preNodes = getPreNodes(root);

    while (walker.nextNode()) {
        node = walker.currentNode;
        parentNodeName = node.parentNode.nodeName;
        if (!isContainedInAny(node, preNodes) &&
            parentNodeName !== "CODE" &&
            parentNodeName !== "STYLE" &&
            parentNodeName !== "SCRIPT"
        ) {
            nodes.push(node);
        }
    }

    return nodes;
}

function transformText() {
    const replacements = [
        { r: /``/g, s: "“" },
        { r: /''/g, s: "”" },

        // Decades, e.g. ’80s - may sometimes be wrong if it encounters a quote
        // that starts with a decade, e.g. '80s John Travolta was awesome.'
        { r: /['‘](\d\d)s/g, s: "’$1s" },

        // Order of these is imporant – opening quotes need to be done first.
        { r: /`/g, s: "‘" },
        { r: /(^|\s|\()"/g, s: "$1“" }, // ldquo
        { r: /"/g,       s: "”" },   // rdquo

        { r: /(^|\s|\()'/g, s: "$1‘" }, // lsquo
        { r: /'/g,       s: "’" },   // rsquo

        // Dashes
        // \u2009 = thin space
        // \u200a = hair space
        // \u2013 = en dash
        // \u2014 = em dash
        { r: /\b\u2013\b/g, s: "\u200a\u2013\u200a" },
        { r: /\b\u2014\b/g, s: "\u200a\u2014\u200a" },
        { r: / \u2014 /g, s: "\u200a\u2014\u200a" },
        { r: /---/g, s: "\u200a\u2014\u200a" },
        { r: / - | -- /g, s: "\u2009\u2013\u2009" },
        { r: /--/g,  s: "\u200a\u2013\u200a" },

        // Stupid things nobody should type.
        { r: /\(no pun intended\)/, s: "" },
        { r: /\(pun intended\)/, s: "" },
        { r: /alot/g, s: "a lot" },

        { r: /\.\.\./g, s: "…" } // hellip
    ];
    const inlineElements = [
        "A",
        "SPAN",
        "EM",
        "I",
        "STRONG",
        "B",
        "SUP",
        "SUB"
    ];
    const root = typeof ROOT_ELEMENT === "undefined" ? null : ROOT_ELEMENT;

    if (typeof root === "string") {
        root = window.document.querySelector(root);
    }
    const textNodes = getTextNodes(root || window.document);

    textNodes.forEach(function (node) {
        const text = node.nodeValue;
        const prev = node.previousSibling;

        // Insert zero-width space character before node text if node
        // immediately follows an inline element, to handle cases like:

        // <p>Link to <a href="blog.com">person</a>'s blog.</p>

        // This prevents the apostrophe being incorrectly replaced with an
        // opening single quote.
        if (prev && inlineElements.indexOf(prev.nodeName) !== -1) {
            text = "\u200B" + text;
        }

        replacements.forEach(function (r) {
            text = text.replace(r.r, r.s);
        });

        node.nodeValue = text;
    });
}

function removeFontStyles() {
    const elements = document.body.querySelectorAll('[style]');
    [].forEach.call(elements, function (el) {
        const styles = el.style.cssText.split(';');
        el.style.cssText = styles.filter(function (style) {
            return !(/font-family|font-size/.test(style));
        }).join(';');
    });
}

(function () {
    "use strict";

    const D = window.document;

    if (D.readyState === "loading") {
        D.addEventListener('DOMContentLoaded', function () {
            transformText();
            removeFontStyles();
        });
    } else {
        // Transform immediately if document has already finished loading, like
        // when inserting this script into an existing document.
        transformText();
        removeFontStyles();
    }
}());
