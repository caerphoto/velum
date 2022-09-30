# Velum

A very simple blog engine: it reads your page templates and Markdown posts and
combines them into a full blog site, which it serves either directly or via
something like nginx. The idea is to be as minimal and lightweight as possible,
while still providing a decent blog experience for your readers.

Live example: <https://blog.andyf.me/> running behind Nginx.

## In-memory Storage

For performance reasons, rather than load the article file from the filesystem
each time it's needed, the articles are read once on startup from
`content/articles` and stored in memory. The title and tags are also extracted
from each article, using the assumption that the first line is of the format `#
Article Title`, and tags are on the second line, in the form `|these, are,
some, tags|`. A 'slug', i.e. a simplified version of the title, is also
generated, for use in URL routing, along with a timestamp, and all of these are
stored with the title-less content.

## Comments

There is a fairly basic commenting system in place that simply stores comment
author name, their URL (optional), and a plain-text comment limited to 3000
characters (not configurable yet).

While comments are also stored in memory while the server is running, they are
also backed up to a file (`content/comments.jsonl` by default) so they can be
restored if the server needs to be restarted. The format, JSONL, is
a line-based variant of JSON – each line is its own independent JSON object
representing a single comment.

Comments are rate-limited by IP address, to prevent some potential abuse. The
limit is 2 seconds, and not currently configurable. There are some other
options I want to explore in this area, including possible cookie-based
'authentication', and more.

Comments are also write-only, and I'm not sure whether this is something I want
to expand on. Making comments editable means implementing a whole system of
user accounts, login, etc., and I'm just not sure it's worth the extra
complication for what's supposed to be a lightweight blog engine.

## Getting started

Assuming you have a functional Rust environment, you can compile and run the
server with `cargo run`. On startup the articles files are read, as described
above, and the Handlebars templates are read from `content/templates`. If
running in development mode, the templates are not cached, and are re-read each
time they're needed. At the moment this setting is controlled directly in the
source code, but in future it will be controllable using a command-line
argument (see To Do, below).

Once the server has started, visit <http://localhost:3090/> in your browser to
see the index page, showing a list of articles. The default is 10 per page,
adjustable via `Settings.toml` along with a couple of other things.

## Running in production

Production use is basically the same as development use, at present. In the
future there may be options for running daemonised, or as a system service.

## To Do

1. Image uploader/manager for admin page.

2. Ensure there are no duplicate slugs.

3. RSS feeds for main site, and maybe per-tag?

4. Themes!
