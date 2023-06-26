// import {chromium} from 'playwright';  // Or 'chromium' or 'firefox'.
const {chromium, webkit, firefox} = require('playwright');

// console.log(chromium);

async function sleep(secs) {
    const p = new Promise(
        (resolve, reject) => {
            setTimeout(() => {
                resolve();
            }, secs * 1000);
        }
    );
    return p;
}

(async () => {

    const context = await webkit.launchPersistentContext('./data', {
    // const context = await chromium.launch({
        headless: false,
        // executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        args: [
            // '--content-shell-hide-toolbar',
            // '--no-first-run',
            // '--disable-infobars',
        ],
        ignoreDefaultArgs: [
            '--enable-automation',
            '--no-sandbox',
            '--disable-breakpad',
            '--use-mock-keychain',
        ],
        viewport: {
            height: 2160,
            width: 3840,
        },
    });
    // console.log(browser);
    // const context = await browser.newContext();
    // const page = await context.pages()[0];
    const page = await context.newPage();
    await page.goto('https://browserleaks.com');
    // await page.screenshot({path: 'screenshot.png'});


    await sleep(5000);

    await context.close();
})();


// const {test} = require('@playwright/test')
// // import {test} from "@playwright/test"
//
// test.describe("Google", () => {
//     test("home", async ({page}) => {
//         await page.goto('https://google.com')
//     })
// })