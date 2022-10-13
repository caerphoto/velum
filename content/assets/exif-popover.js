(function () {
  let exifPopover;

  function positionFromImage(img) {
    const clientRect = img.getBoundingClientRect();
    const popoverRect = exifPopover.getBoundingClientRect();

    return {
      left: clientRect.left + window.scrollX,
      top: clientRect.top + clientRect.height - popoverRect.height +
        window.scrollY
    };
  }

  function showExifOnImage(event) {
    const img = event.target;

    if (img.nodeName !== 'IMG') {
      return;
    }
    exifPopover.classList.add('active');
    exifPopover.classList.add('loading');
    const popoverPosition = positionFromImage(img);

    exifPopover.style.top = popoverPosition.top + 'px';
    exifPopover.style.left = popoverPosition.left + 'px';

    window.EXIF.getData(img, function () {
      const data = this.exifdata;
      if (Object.keys(data).length === 0) {
        return;
      }
      let model = data.LensModel || '(lens name unavailable)';
      const length = Math.round(data.FocalLength) || '--';
      const aperture = data.FNumber || '--';

      model = model.replace('Fujifilm Fujinon', '');

      exifPopover.innerHTML = [
        length + 'mm',
        'f/' + aperture
      ].join(' &middot; ') +
        '<br><span class="lens-name">' + model + '</span>';

      exifPopover.classList.remove('loading');
    });
  }

  function hideExif() {
    exifPopover.classList.remove('active');
  }

  if (window.EXIF) {
    exifPopover = document.createElement('div');
    exifPopover.id = 'exif-popover';
    document.body.appendChild(exifPopover);
    document.body.addEventListener('mouseover', showExifOnImage);
    document.body.addEventListener('mouseout', hideExif);
  }
}());
