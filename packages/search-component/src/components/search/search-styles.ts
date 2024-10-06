import styled from 'styled-components'

export const SearchContainer = styled.div`
  position: relative;
`

export const SearchInput = styled.input`
  padding: 0.5em;

  font-size: 1.5em;

  border: none;
  border-bottom: 1px solid black;
`

export const SearchResults = styled.ul`
  position: absolute;
  z-index: 1;

  display: flex;
  flex-direction: column;
  width: 100%;
  margin: 0;
  padding: 0;

  list-style: none;
  background-color: white;
`

export const SearchResult = styled.li`
  display: inline-flex;
  flex-direction: column;
  padding: 0.5em;

  background-color: white;
  border-bottom: 1px solid black;
`
