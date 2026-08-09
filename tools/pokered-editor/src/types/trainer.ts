export const TRAINER_CLASSES = [
  'Nobody', 'Youngster', 'BugCatcher', 'Lass', 'Sailor',
  'JrTrainerM', 'JrTrainerF', 'Pokemaniac', 'SuperNerd', 'Hiker',
  'Biker', 'Burglar', 'Engineer', 'UnusedJuggler', 'Fisher',
  'Swimmer', 'CueBall', 'Gambler', 'Beauty', 'PsychicTr',
  'Rocker', 'Juggler', 'Tamer', 'BirdKeeper', 'Blackbelt',
  'Rival1', 'ProfOak', 'Chief', 'Scientist', 'Giovanni',
  'Rocket', 'CooltrainerM', 'CooltrainerF', 'Bruno', 'Brock',
  'Misty', 'LtSurge', 'Erika', 'Koga', 'Blaine',
  'Sabrina', 'Gentleman', 'Rival2', 'Rival3', 'Lorelei',
  'Channeler', 'Agatha', 'Lance',
] as const

export type TrainerClass = typeof TRAINER_CLASSES[number]

export interface TrainerMon {
  level: number
  species: string
}

export interface TrainerParty {
  pokemon: TrainerMon[]
}

export interface TrainerClassFile {
  class: TrainerClass
  constName: string
  parties: TrainerParty[]
}
