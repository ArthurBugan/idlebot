//! Smart Contracts — Solidity

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./USDTInterface.sol";

/**
 * @title TemplateMarket
 * @notice Marketplace for GitHub templates/repositories
 * @dev Players publish templates, others buy with USDT
 */
contract TemplateMarket {
    USDTInterface public usdt;

    struct Listing {
        address seller;
        string title;
        string githubUrl;
        string description;
        uint256 priceUsdt; // Price in USDT (6 decimals)
        bool sold;
        uint256 publishedAt;
    }

    uint256 public nextListingId;
    mapping(uint256 => Listing) public listings;

    // Fee: 5%
    uint256 public constant FEE_PERCENT = 5;
    address public feeRecipient;

    event ListingCreated(
        uint256 indexed listingId,
        address indexed seller,
        string title,
        uint256 priceUsdt
    );

    event ListingSold(
        uint256 indexed listingId,
        address indexed seller,
        address indexed buyer,
        uint256 priceUsdt
    );

    constructor(address _usdt, address _feeRecipient) {
        usdt = USDTInterface(_usdt);
        feeRecipient = _feeRecipient;
        nextListingId = 1;
    }

    function publishListing(
        string calldata _title,
        string calldata _githubUrl,
        string calldata _description,
        uint256 _priceUsdt
    ) external {
        require(_priceUsdt >= 10_000, "Price too low (min 0.01 USDT)");
        require(bytes(_githubUrl).length > 0, "Invalid URL");

        uint256 listingId = nextListingId++;

        listings[listingId] = Listing({
            seller: msg.sender,
            title: _title,
            githubUrl: _githubUrl,
            description: _description,
            priceUsdt: _priceUsdt,
            sold: false,
            publishedAt: block.timestamp
        });

        emit ListingCreated(listingId, msg.sender, _title, _priceUsdt);
    }

    function purchaseListing(uint256 _listingId) external {
        Listing storage listing = listings[_listingId];
        require(!listing.sold, "Already sold");
        require(msg.sender != listing.seller, "Cannot buy own listing");

        uint256 price = listing.priceUsdt;
        uint256 fee = (price * FEE_PERCENT) / 100;
        uint256 sellerAmount = price - fee;

        require(
            usdt.transferFrom(msg.sender, address(this), price),
            "USDT transfer failed"
        );

        require(
            usdt.transfer(listing.seller, sellerAmount),
            "Seller payment failed"
        );

        if (fee > 0) {
            require(
                usdt.transfer(feeRecipient, fee),
                "Fee transfer failed"
            );
        }

        listing.sold = true;

        emit ListingSold(
            _listingId,
            listing.seller,
            msg.sender,
            price
        );
    }

    function withdraw(uint256 _listingId) external {
        Listing storage listing = listings[_listingId];
        require(msg.sender == listing.seller, "Not seller");
        require(listing.sold, "Not sold yet");

        uint256 fee = (listing.priceUsdt * FEE_PERCENT) / 100;
        uint256 amount = listing.priceUsdt - fee;

        require(
            usdt.transfer(msg.sender, amount),
            "Withdrawal failed"
        );

        listing.sold = false;
    }

    function getListing(uint256 _listingId) external view returns (Listing memory) {
        return listings[_listingId];
    }

    function getActiveListings() external view returns (uint256[] memory) {
        uint256 count = 0;
        for (uint256 i = 1; i < nextListingId; i++) {
            if (!listings[i].sold) count++;
        }

        uint256[] memory result = new uint256[](count);
        uint256 idx = 0;
        for (uint256 i = 1; i < nextListingId; i++) {
            if (!listings[i].sold) {
                result[idx++] = i;
            }
        }
        return result;
    }
}
