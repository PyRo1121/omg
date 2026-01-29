import { createHandler, StartServer } from "@solidjs/start/server";

export default createHandler(() => (
  <StartServer
    document={({ assets, children, scripts }) => (
      <html lang="en">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
          <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
          <link rel="manifest" href="/site.webmanifest" />
          <meta name="theme-color" content="#0a0a0a" />
          
          {/* Preconnects */}
          <link rel="preconnect" href="https://api.pyro1121.com" />
          
          {/* Structured Data */}
          <script type="application/ld+json">{`
            {
              "@context": "https://schema.org",
              "@type": "SoftwareApplication",
              "name": "OMG Package Manager",
              "applicationCategory": "DeveloperApplication",
              "operatingSystem": "Linux",
              "description": "Fastest unified package manager for Arch Linux, Debian, and Ubuntu. Native Node.js, Python, Go, Rust, Ruby, Java, and Bun support. 22x faster than pacman.",
              "url": "https://pyro1121.com",
              "offers": {
                "@type": "Offer",
                "price": "0",
                "priceCurrency": "USD"
              },
              "aggregateRating": {
                "@type": "AggregateRating",
                "ratingValue": "4.8",
                "ratingCount": "250"
              }
            }
          `}</script>
          
          {assets}
        </head>
        <body class="bg-[#0a0a0a] text-white antialiased">
          <div id="app">{children}</div>
          {scripts}
        </body>
      </html>
    )}
  />
));
