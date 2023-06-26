# Using Sync Methods
from playwright.sync_api import sync_playwright
with sync_playwright() as Page:

    #To Launch Default Chromium Browser
    Browser = Page.chromium.launch(headless=False)
    page = Browser.new_page()
    page.goto("https://www.google.com")

    #To Launch Edge Browser
    Browser = Page.chromium.launch(headless=False,channel="msedge")
    page= Browser.new_page()
    page.goto("https://www.google.com")

    #To Launch Chrome Browser
    Browser = Page.chromium.launch(headless=False, channel="chrome")
    page = Browser.new_page()
    page.goto("https://www.google.com")

    #To Launch Firefox Browser
    Browser = Page.chromium.launch(headless=False,channel="firefox")
    page = Browser.new_page()
    page.goto("https://www.google.com") 