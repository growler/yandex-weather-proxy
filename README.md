A simple HTTP server that does exactly two things:

- fetches [Yandex.Weather](https://yandex.com/dev/weather/doc/dg/concepts/forecast-info.html)
  forecast data and exposes it at "/weather.json" URL. The server caches forecast data to
  avoid exhausting free Yandex.Weather request limit of 50 requests per day.
- fetches Yandex.Weather icons, converts these from SVG to PNG format (using librsvg2 
  rsvg-convert binary) and exposes they at "/icon/[name].[res].png" URL, where "[res]"
  is PNG width.

I use this server together with [Iridium 7" Panel](https://iridi.com/panel/) and 
iridi application to display weather forecast. 

Thanks to Rust, the server consumes very little resources and perfectly runs on a
ARMv7 [Wirenboard Controller](https://wirenboard.com/en/catalog/kontrollery/)
