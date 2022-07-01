# Velum

A very simple blog engine: it reads your page templates and Markdown posts and combines them into a full blog site, which it serves either directly or via something like nginx. The idea is to be as minimal and lightweight as possible, while still providing a decent blog experience for your readers.

## Requires Redis

For performance reasons, rather than load the article file from the filesystem each time it's needed, the articles are read once on startup from `content/articles` and stored in a Redis database. The title is also extracted from each article, using the assumption that the first line is of the format `# Article Title`. A 'slug', i.e. a simplified version of the title, is also generated, for use in URL routing, along with a timestamp, and all of these are stored in Redis with the title-less content.

Note that other than separating the title from the content, no other processing is done to the Markdown files; they're only converted to HTML upon request. This may change in the future, if the flexibility offered by on-demand conversion proves unnecessary.

## Getting started

Assuming you have a functional Rust environment, you can compile and run the server with `cargo run`. On startup the articles files are read, as described above, and the Handlebars templates are read from `content/templates`. If running in development mode, the templates are not cached, and are re-read each time they're needed. At the moment this setting is controlled directly in the source code, but in future it will be controllable using a command-line argument.

Once the server has started, visit <http://localhost:3090/> in your browser to see the index page, showing a list of articles. Currently it's fixed at 10 per page, but eventually this will be a configurable setting.

## Running in production

Production use is basically the same as development use, at present. In the future there may be options for running daemonised, or as a system service.

## To Do

1. Highest on the priority list is a tag system, but implementation requires some consideration. Markdown files don't have an obvious or standard way to include tags, so there's no automated way to generate them; neither is there a readily apparent way to input them manually. The solution I'm considering at the moment is some custom syntax at the end of the Markdown file, maybe like `|a tag, another tag, velum, photography|` – something that's both easy to write and easy to parse.

2. Next up is configuration. At present a lot of stuff is hard-coded, when it really should be configurable. Based on the brief research I've done on this, implementing this shouldn't be too difficult, but should provide significant benefits.

3. After that there's the problem of rebuilding the article database – restarting the server is a simple option, but obviously means downtime, albeit very brief. A better option would be a way to send a special HTTP request to the server that signals it to restart, possibly via an admin page of some kind.

4. Finally there's the question of an editor: does Velum even need one? If not, what about a simplified way to upload content, that means users don't have to manually copy files (including images) to their server? This is rather an open question at the moment, something I'll need to think on a lot more.
