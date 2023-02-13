(function() {
    if (!window.EXIF) return;
    let exifPopover;

    function positionFromImage(img) {
        const clientRect = img.getBoundingClientRect();
        const popoverRect = exifPopover.getBoundingClientRect();

        return {
            left: clientRect.left + window.scrollX,
            top: clientRect.top + clientRect.height - popoverRect.height +
                window.scrollY,
        };
    }

    function hasLoaded(img) {
        return img.complete &&
            img.naturalHeight > 0 &&
            img.naturalWidth > 0;
    }

    function showExifOnImage(event) {
        const img = event.target;

        if (img.nodeName !== "IMG") return;
        if (!hasLoaded(img)) return;

        const popoverPosition = positionFromImage(img);

        exifPopover.style.top = popoverPosition.top + "px";
        exifPopover.style.left = popoverPosition.left + "px";

        window.EXIF.getData(img, function() {
            const data = this.exifdata;
            if (Object.keys(data).length === 0) {
                return;
            }
            if (!data.FocalLength && !data.FNumber && !data.LensModel) {
                return;
            }

            let model = data.LensModel || "(lens name unavailable)";
            model = model.replace("Fujifilm Fujinon", "");
            const length = Math.round(data.FocalLength) || "--";
            const aperture = data.FNumber || "--";

            exifPopover.innerHTML = [
                length + "mm",
                "f/" + aperture,
            ].join(" &middot; ") +
                '<br><span class="lens-name">' + model + "</span>";
            exifPopover.classList.add("active");
        });
    }

    function hideExif() {
        exifPopover.classList.remove("active");
    }

    exifPopover = document.createElement("div");
    exifPopover.innerHTML = "--<br>--";
    exifPopover.id = "exif-popover";
    document.body.appendChild(exifPopover);
    document.body.addEventListener("mouseover", showExifOnImage);
    document.body.addEventListener("mouseout", hideExif);

    document.body.addEventListener("click", function(event) {
        setTimeout(
            function() {
                if (this.target.nodeName !== "IMG") return;
                if (this.target.classList.contains("focus")) {
                    if (exifPopover.classList.contains("active")) {
                        hideExif();
                    } else {
                        showExifOnImage(this);
                    }
                }
            }.bind(event),
            0,
        );
    });
})();
