import styled from "styled-components";

export const SearchContainer = styled.div`
    position: relative;
    font-family: system-ui, sans-serif;
`;

export const SearchResults = styled.ul`
    position: absolute;
    z-index: 1;

    display: flex;
    flex-direction: column;
    width: 100%;
    height: min-content;
    max-height: 400px;
    margin: 0;
    padding: 0;
    overflow: hidden;
    overflow-y: auto;

    list-style: none;
    background-color: white;
`;

export const SearchResult = styled.li`
    display: grid;
    grid-template-rows: auto;
    grid-template-columns: 1fr 1fr;
    margin: 0;
    padding: 1rem 0;
`;

export const ResultMatch = styled.div`
    grid-row: 1 / 1;
    grid-column: 1 / 3;
    padding: 0.5em;

    font-size: 1em;
`;

export const ResultImage = styled.img`
    grid-row: 1 / 1;
    grid-column: 1 / 2;
    width: 100px;
    height: 100px;
    padding: 0.5em;
    object-fit: cover;
`;

export const ResultSource = styled.div`
    grid-row: 2/3;
    grid-column: 1/2;
    padding: 0.5em;

    font-weight: 600;
    font-size: 0.8em;
`;

export const ResultTimestamp = styled.div`
    grid-row: 2/3;
    grid-column: 2/3;
    padding: 0.5em;

    color: grey;
    font-weight: 600;
    font-size: 0.8em;
`;

export const SearchInput = styled.input`
    padding: 0.5em;

    font-size: 1.5em;

    border: none;
    border-bottom: 1px solid black;
`;
