'use client'

import styled from 'styled-components'

export const Header = styled.h1`
  margin: 0;
  padding: 1rem;

  color: red;
  font-size: 3rem;
`

export const Container = styled.figure`
  display: grid;
  grid-template-rows: auto;
  grid-template-columns: 3fr 1fr;
  gap: 0.5rem;
  margin: 2rem;
`

export const PlayerColumn = styled.div`
  position: relative;
`
