import styled from "styled-components";

const SearchField = styled.input`
    padding: 0.5em;

    font-size: 1.5em;

    border: none;
    border-bottom: 1px solid black;
`;

export function SearchInput(props: any) {
    return <SearchField { ...props } />;
}
