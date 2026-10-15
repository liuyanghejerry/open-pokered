# Battle PP restore and Transform identity primary evidence

Reference source fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c; ROM SHA256 5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b.

The chooser reads wPartyMon1Moves; GetMaxPP is called with PLAYER_PARTY_DATA; restoration writes party PP. On successful restore to the active index, all four packed party PP bytes are copied to wBattleMonPP. Transform writes battle species/types/moves/stats/PP while persistent party moves remain Transform. Thus Ether selects/restores Transform after Transform, retaining copied battle moves and copying only restored PP back. Failure does not run the copy. This is distinct from regular selection of battle moves.


## engine/items/item_effects.asm:1954–2079
SHA256 `76b570b5216bbbcf6d9528c4704d931744a3740cce88afdc4f4f8f700449dda1`

```asm
1954: ItemUsePPRestore:
1955: 	ld a, [wWhichPokemon]
1956: 	push af
1957: 	ld a, [wCurItem]
1958: 	ld [wPPRestoreItem], a
1959: .chooseMon
1960: 	xor a
1961: 	ld [wUpdateSpritesEnabled], a
1962: 	ld a, USE_ITEM_PARTY_MENU
1963: 	ld [wPartyMenuTypeOrMessageID], a
1964: 	call DisplayPartyMenu
1965: 	jr nc, .chooseMove
1966: 	jp .itemNotUsed
1967: .chooseMove
1968: 	ld a, [wPPRestoreItem]
1969: 	cp ELIXER
1970: 	jp nc, .useElixir ; if Elixir or Max Elixir
1971: 	ld a, $02
1972: 	ld [wMoveMenuType], a
1973: 	ld hl, RaisePPWhichTechniqueText
1974: 	ld a, [wPPRestoreItem]
1975: 	cp ETHER ; is it a PP Up?
1976: 	jr c, .printWhichTechniqueMessage ; if so, print the raise PP message
1977: 	ld hl, RestorePPWhichTechniqueText ; otherwise, print the restore PP message
1978: .printWhichTechniqueMessage
1979: 	call PrintText
1980: 	xor a
1981: 	ld [wPlayerMoveListIndex], a
1982: 	callfar MoveSelectionMenu ; move selection menu
1983: 	ld a, 0
1984: 	ld [wPlayerMoveListIndex], a
1985: 	jr nz, .chooseMon
1986: 	ld hl, wPartyMon1Moves
1987: 	ld bc, PARTYMON_STRUCT_LENGTH
1988: 	call GetSelectedMoveOffset
1989: 	push hl
1990: 	ld a, [hl]
1991: 	ld [wNamedObjectIndex], a
1992: 	call GetMoveName
1993: 	call CopyToStringBuffer
1994: 	pop hl
1995: 	ld a, [wPPRestoreItem]
1996: 	cp ETHER
1997: 	jr nc, .useEther ; if Ether or Max Ether
1998: ; use PP Up
1999: 	ld bc, MON_PP - MON_MOVES
2000: 	add hl, bc
2001: 	ld a, [hl] ; move PP
2002: 	cp 3 << 6 ; have 3 PP Ups already been used?
2003: 	jr c, .PPNotMaxedOut
2004: 	ld hl, PPMaxedOutText
2005: 	call PrintText
2006: 	jr .chooseMove
2007: .PPNotMaxedOut
2008: 	ld a, [hl]
2009: 	add 1 << 6 ; increase PP Up count by 1
2010: 	ld [hl], a
2011: 	ld a, 1 ; 1 PP Up used
2012: 	ld [wUsingPPUp], a
2013: 	call RestoreBonusPP ; add the bonus PP to current PP
2014: 	ld hl, PPIncreasedText
2015: 	call PrintText
2016: .done
2017: 	pop af
2018: 	ld [wWhichPokemon], a
2019: 	call GBPalWhiteOut
2020: 	call RunDefaultPaletteCommand
2021: 	jp RemoveUsedItem
2022: .afterRestoringPP ; after using a (Max) Ether/Elixir
2023: 	ld a, [wWhichPokemon]
2024: 	ld b, a
2025: 	ld a, [wPlayerMonNumber]
2026: 	cp b ; is the pokemon whose PP was restored active in battle?
2027: 	jr nz, .skipUpdatingInBattleData
2028: 	ld hl, wPartyMon1PP
2029: 	ld bc, PARTYMON_STRUCT_LENGTH
2030: 	call AddNTimes
2031: 	ld de, wBattleMonPP
2032: 	ld bc, NUM_MOVES
2033: 	call CopyData ; copy party data to in-battle data
2034: .skipUpdatingInBattleData
2035: 	ld a, SFX_HEAL_AILMENT
2036: 	call PlaySound
2037: 	ld hl, PPRestoredText
2038: 	call PrintText
2039: 	jr .done
2040: .useEther
2041: 	call .restorePP
2042: 	jr nz, .afterRestoringPP
2043: 	jp .noEffect
2044: ; unsets zero flag if PP was restored, sets zero flag if not
2045: ; however, this is bugged for Max Ethers and Max Elixirs (see below)
2046: .restorePP
2047: 	xor a ; PLAYER_PARTY_DATA
2048: 	ld [wMonDataLocation], a
2049: 	call GetMaxPP
2050: 	ld hl, wPartyMon1Moves
2051: 	ld bc, PARTYMON_STRUCT_LENGTH
2052: 	call GetSelectedMoveOffset
2053: 	ld bc, MON_PP - MON_MOVES
2054: 	add hl, bc ; hl now points to move's PP
2055: 	ld a, [wMaxPP]
2056: 	ld b, a
2057: 	ld a, [wPPRestoreItem]
2058: 	cp MAX_ETHER
2059: 	jr z, .fullyRestorePP
2060: 	ld a, [hl] ; move PP
2061: 	and PP_MASK
2062: 	cp b ; does current PP equal max PP?
2063: 	ret z ; if so, return
2064: 	add 10 ; increase current PP by 10
2065: ; b holds the max PP amount and b will hold the new PP amount.
2066: ; So, if the new amount meets or exceeds the max amount,
2067: ; cap the amount to the max amount by leaving b unchanged.
2068: ; Otherwise, store the new amount in b.
2069: 	cp b ; does the new amount meet or exceed the maximum?
2070: 	jr nc, .storeNewAmount
2071: 	ld b, a
2072: .storeNewAmount
2073: 	ld a, [hl] ; move PP
2074: 	and PP_UP_MASK
2075: 	add b
2076: 	ld [hl], a
2077: 	ret
2078: .fullyRestorePP
2079: 	ld a, [hl] ; move PP
```


## engine/items/item_effects.asm:2467–2537
SHA256 `76b570b5216bbbcf6d9528c4704d931744a3740cce88afdc4f4f8f700449dda1`

```asm
2467: GetMaxPP:
2468: 	ld a, [wMonDataLocation]
2469: 	and a
2470: 	ld hl, wPartyMon1Moves
2471: 	ld bc, PARTYMON_STRUCT_LENGTH
2472: 	jr z, .sourceWithMultipleMon
2473: 	ld hl, wEnemyMon1Moves
2474: 	dec a
2475: 	jr z, .sourceWithMultipleMon
2476: 	ld hl, wBoxMon1Moves
2477: 	ld bc, BOXMON_STRUCT_LENGTH
2478: 	dec a
2479: 	jr z, .sourceWithMultipleMon
2480: 	ld hl, wDayCareMonMoves
2481: 	dec a
2482: 	jr z, .sourceWithOneMon
2483: 	ld hl, wBattleMonMoves ; player's in-battle pokemon
2484: .sourceWithOneMon
2485: 	call GetSelectedMoveOffset2
2486: 	jr .next
2487: .sourceWithMultipleMon
2488: 	call GetSelectedMoveOffset
2489: .next
2490: 	ld a, [hl]
2491: 	dec a
2492: 	push hl
2493: 	ld hl, Moves
2494: 	ld bc, MOVE_LENGTH
2495: 	call AddNTimes
2496: 	ld de, wMoveData
2497: 	ld a, BANK(Moves)
2498: 	call FarCopyData
2499: 	ld de, wMoveData + MOVE_PP
2500: 	ld a, [de]
2501: 	ld b, a ; b = normal max PP
2502: 	pop hl
2503: 	push bc
2504: 	ld bc, MON_PP - MON_MOVES ; PP offset if not player's in-battle pokemon data
2505: 	ld a, [wMonDataLocation]
2506: 	cp 4 ; player's in-battle pokemon?
2507: 	jr nz, .addPPOffset
2508: 	ld bc, wBattleMonPP - wBattleMonMoves ; PP offset if player's in-battle pokemon data
2509: .addPPOffset
2510: 	add hl, bc
2511: 	ld a, [hl] ; a = current PP
2512: 	and PP_UP_MASK
2513: 	pop bc
2514: 	or b ; place normal max PP in 6 lower bits of a
2515: 	ASSERT wMoveData + MOVE_PP + 1 == wPPUpCountAndMaxPP
2516: 	ld h, d
2517: 	ld l, e
2518: 	inc hl ; hl = wPPUpCountAndMaxPP
2519: 	ld [hl], a
2520: 	xor a ; add the bonus for the existing PP Up count
2521: 	ld [wUsingPPUp], a
2522: 	call AddBonusPP ; add bonus PP from PP Ups
2523: 	ld a, [hl]
2524: 	and PP_MASK
2525: 	ld [wMaxPP], a ; store max PP
2526: 	ret
2527:
2528: GetSelectedMoveOffset:
2529: 	ld a, [wWhichPokemon]
2530: 	call AddNTimes
2531:
2532: GetSelectedMoveOffset2:
2533: 	ld a, [wCurrentMenuItem]
2534: 	ld c, a
2535: 	ld b, 0
2536: 	add hl, bc
2537: 	ret
```


## engine/battle/core.asm:2520–2541
SHA256 `928a66d1ac444fc913c7c658d9f7739eb6351e797b3276da64a5de9682a57a5f`

```asm
2520: .relearnmenu
2521: 	ld a, [wWhichPokemon]
2522: 	ld hl, wPartyMon1Moves
2523: 	ld bc, PARTYMON_STRUCT_LENGTH
2524: 	call AddNTimes
2525: 	call .loadmoves
2526: 	hlcoord 4, 7
2527: 	ld b, 4
2528: 	ld c, 14
2529: 	call TextBoxBorder
2530: 	hlcoord 6, 8
2531: 	call .writemoves
2532: 	ld b, $5
2533: 	ld a, $7
2534: .menuset
2535: 	ld hl, wTopMenuItemY
2536: 	ld [hli], a ; wTopMenuItemY
2537: 	ld a, b
2538: 	ld [hli], a ; wTopMenuItemX
2539: 	ld a, [wMoveMenuType]
2540: 	cp $1
2541: 	jr z, .selectedmoveknown
```


## engine/battle/move_effects/transform.asm:55–142
SHA256 `6e19fb3d091428cde5719477ee22af671bdbb82652326e52f958269f3be1aada`

```asm
55: 	pop hl
56: 	push hl
57: ; transform user into opposing Pokemon
58: ; species
59: 	ld a, [hl]
60: 	ld [de], a
61: ; type 1, type 2, catch rate, and moves
62: 	ld bc, $5
63: 	add hl, bc
64: 	inc de
65: 	inc de
66: 	inc de
67: 	inc de
68: 	inc de
69: 	inc bc
70: 	inc bc
71: 	call CopyData
72: 	ldh a, [hWhoseTurn]
73: 	and a
74: 	jr z, .next
75: ; save enemy mon DVs at wTransformedEnemyMonOriginalDVs
76: 	ld a, [de]
77: 	ld [wTransformedEnemyMonOriginalDVs], a
78: 	inc de
79: 	ld a, [de]
80: 	ld [wTransformedEnemyMonOriginalDVs + 1], a
81: 	dec de
82: .next
83: ; DVs
84: 	ld a, [hli]
85: 	ld [de], a
86: 	inc de
87: 	ld a, [hli]
88: 	ld [de], a
89: 	inc de
90: ; Skip level and max HP
91: 	inc hl
92: 	inc hl
93: 	inc hl
94: 	inc de
95: 	inc de
96: 	inc de
97: ; Attack, Defense, Speed, and Special stats
98: 	ld bc, (NUM_STATS - 1) * 2
99: 	call CopyData
100: 	ld bc, wBattleMonMoves - wBattleMonPP
101: 	add hl, bc ; ld hl, wBattleMonMoves
102: 	ld b, NUM_MOVES
103: .copyPPLoop
104: ; 5 PP for all moves
105: 	ld a, [hli]
106: 	and a
107: 	jr z, .lessThanFourMoves
108: 	ld a, 5
109: 	ld [de], a
110: 	inc de
111: 	dec b
112: 	jr nz, .copyPPLoop
113: 	jr .copyStats
114: .lessThanFourMoves
115: ; 0 PP for blank moves
116: 	xor a
117: 	ld [de], a
118: 	inc de
119: 	dec b
120: 	jr nz, .lessThanFourMoves
121: .copyStats
122: ; original (unmodified) stats and stat mods
123: 	pop hl
124: 	ld a, [hl]
125: 	ld [wNamedObjectIndex], a
126: 	call GetMonName
127: 	ld hl, wEnemyMonUnmodifiedAttack
128: 	ld de, wPlayerMonUnmodifiedAttack
129: 	call .copyBasedOnTurn ; original (unmodified) stats
130: 	ld hl, wEnemyMonStatMods
131: 	ld de, wPlayerMonStatMods
132: 	call .copyBasedOnTurn ; stat mods
133: 	ld hl, TransformedText
134: 	jp PrintText
135:
136: .copyBasedOnTurn
137: 	ldh a, [hWhoseTurn]
138: 	and a
139: 	jr z, .gotStatsOrModsToCopy
140: 	push hl
141: 	ld h, d
142: 	ld l, e
```
