(function() {
    if (!window.EXIF) return;
    let exifPopover = document.createElement("div");

    function positionFromImage(img) {
        const imgRect = img.getBoundingClientRect();
        const popoverRect = exifPopover.getBoundingClientRect();

        return {
            left: imgRect.left + window.scrollX,
            top: imgRect.top + imgRect.height - popoverRect.height + window.scrollY,
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

        exifPopover.classList.add("active");
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

            let lensName = data.LensModel || "(lens name unavailable)";
            let cameraName = data.Model;
            lensName = lensName.replace("Fujifilm Fujinon", "");
            const length = Math.round(data.FocalLength) || "--";
            const aperture = data.FNumber || "--";

            exifPopover.innerHTML =
                `<span class="exif-data">${length}mm</span> at
                <span class="exif-data">f/${aperture}</span><br>
                <span class="exif-data">${lensName}</span> on
                <span class="exif-data">${cameraName}</span>`;
        });
    }

    function hideExif() {
        exifPopover.classList.remove("active");
    }

    exifPopover.id = "exif-popover";
    exifPopover.innerHTML = "--<br>--";
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
