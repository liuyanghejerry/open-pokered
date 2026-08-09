// ───────────────────────────────────────────────────────────────────────────
// presets.ts — motion preset catalog (generated from PerfectPixel's presets.go).
//
// 100 animation keywords; each maps a state name → choreography `hint` injected
// into the strip prompt. `hint` is backend-only (stripped before sending the
// catalog to the browser via listPresets()). Categories were mapped from the
// original Korean to English; labels are prettified from the name.
// ───────────────────────────────────────────────────────────────────────────

export interface PresetInfo {
  name: string
  label: string
  category: string
  action: string
  frames: number
  fps: number
  loop: boolean
  /** Motion guide injected into the generation prompt (backend only). */
  hint: string
}

export const Presets: PresetInfo[] = [
  // ── Basic ──
  { name: 'idle', label: 'Idle', category: 'Basic', action: 'subtle breathing idle standing in place', frames: 4, fps: 6, loop: true, hint: 'Subtle in-place breathing cycle: gentle chest rise and fall, tiny up-down body shift of a few pixels, occasional blink. Feet stay planted in the same spot in every frame.' },
  { name: 'idle-combat', label: 'Idle Combat', category: 'Basic', action: 'ready combat stance, weapon up, weight shifting', frames: 4, fps: 8, loop: true, hint: 'Alert combat-ready idle: knees slightly bent, weapon or fists raised, weight shifting subtly side to side, small breathing bob. Feet stay planted; stance never relaxes.' },
  { name: 'walk', label: 'Walk', category: 'Basic', action: 'side-view walking cycle facing right', frames: 6, fps: 10, loop: true, hint: 'Readable side-view walking cycle: alternating legs with clear contact and passing poses, opposite arm swing, slight body bob. Each frame shows a distinctly different leg position.' },
  { name: 'run', label: 'Run', category: 'Basic', action: 'fast side-view running cycle facing right', frames: 6, fps: 12, loop: true, hint: 'Fast side-view running cycle: strong forward lean, large leg extension with airborne moments, pumping arms, pronounced body bob. Each frame is a distinct stride phase.' },
  { name: 'sprint', label: 'Sprint', category: 'Basic', action: 'all-out sprint, extreme lean and stride', frames: 6, fps: 14, loop: true, hint: 'All-out sprint: extreme forward lean, maximal leg extension, both feet airborne at peak, arms pumping hard. Faster, larger strides than a normal run.' },
  { name: 'jump', label: 'Jump', category: 'Basic', action: 'crouch, take off, airborne peak, land', frames: 5, fps: 10, loop: false, hint: 'Jump sequence: crouching anticipation, take-off with body extended upward, airborne peak with legs tucked, landing recovery crouch. Vary the body\'s vertical position to show the arc.' },
  { name: 'fall', label: 'Fall', category: 'Basic', action: 'falling through the air', frames: 4, fps: 10, loop: true, hint: 'Falling cycle: body airborne, arms and legs flailing or bracing, slight rotation or wobble, hair and clothes pushed upward by wind. No ground contact in any frame.' },
  { name: 'land', label: 'Land', category: 'Basic', action: 'land from a fall and absorb impact', frames: 4, fps: 12, loop: false, hint: 'Landing impact: feet touch down, deep knee bend to absorb shock, body compresses low, then rises back toward standing. Show the compression clearly in the middle frame.' },
  { name: 'crouch', label: 'Crouch', category: 'Basic', action: 'lower into a compact crouch and hold', frames: 4, fps: 8, loop: false, hint: 'Crouching sequence: from standing, bend knees and lower the body progressively into a compact crouch, head tucked slightly. Final frame is fully crouched.' },
  { name: 'crawl', label: 'Crawl', category: 'Basic', action: 'crawl forward on hands and knees', frames: 6, fps: 8, loop: true, hint: 'Hands-and-knees crawling cycle: alternating arm-and-opposite-leg reaches, low body close to the ground, head up. Each frame a distinct crawl phase.' },
  { name: 'climb', label: 'Climb', category: 'Basic', action: 'climb up a vertical surface', frames: 6, fps: 8, loop: true, hint: 'Vertical climbing cycle: alternating hand-over-hand reaches and matching foot pushes, body pressed close to the surface, upward progress implied. Each frame a distinct reach.' },
  { name: 'swim', label: 'Swim', category: 'Basic', action: 'swimming stroke cycle', frames: 6, fps: 8, loop: true, hint: 'Swimming stroke cycle: arms reaching forward and pulling back in alternation, legs kicking, body horizontal. Each frame a distinct stroke phase.' },
  { name: 'dash', label: 'Dash', category: 'Basic', action: 'quick burst dash forward', frames: 4, fps: 14, loop: false, hint: 'Quick dash burst: explosive crouch-and-push start, body stretched low and forward at peak speed, then a brief settle. Strong horizontal lean throughout.' },
  { name: 'roll', label: 'Roll', category: 'Basic', action: 'evasive forward roll', frames: 5, fps: 14, loop: false, hint: 'Evasive forward roll: tuck into a ball, rotate fully over the shoulder, and rise back to a crouch. Show clear rotation phases across frames.' },
  { name: 'slide', label: 'Slide', category: 'Basic', action: 'sliding low along the ground', frames: 4, fps: 12, loop: false, hint: 'Low slide: drop into a feet-first slide with one leg extended, body leaning back low to the ground, then begin to rise. Body stays low across frames.' },
  { name: 'sit', label: 'Sit', category: 'Basic', action: 'sit down to the ground', frames: 4, fps: 8, loop: false, hint: 'Sitting down: bend at knees and hips, lower the body, settle onto the ground in a relaxed seated pose. Final frame clearly seated.' },
  { name: 'sleep', label: 'Sleep', category: 'Basic', action: 'sleeping lying down, gentle breathing', frames: 4, fps: 4, loop: true, hint: 'Sleeping cycle: lying down with eyes closed, slow gentle breathing rise and fall, occasional small shift. Very calm, minimal motion.' },
  { name: 'turn', label: 'Turn', category: 'Basic', action: 'turn around to face the other way', frames: 4, fps: 10, loop: false, hint: 'Turn-around: rotate the body from facing one way to the opposite, weight pivoting on the feet, head leading the turn. Show clear intermediate angles.' },
  // ── Combat ──
  { name: 'attack', label: 'Attack', category: 'Combat', action: 'melee attack with wind-up, strike, recovery', frames: 5, fps: 12, loop: false, hint: 'Melee attack: wind-up with body coiled back, powerful strike at full extension, follow-through, recovery to ready stance. The strike frame is the most extreme pose.' },
  { name: 'attack-heavy', label: 'Attack Heavy', category: 'Combat', action: 'slow heavy melee attack with big wind-up', frames: 6, fps: 10, loop: false, hint: 'Heavy attack: long exaggerated wind-up loading weight back, a slow powerful swing, deep follow-through, slow recovery. Bigger and slower than a normal attack.' },
  { name: 'combo', label: 'Combo', category: 'Combat', action: 'multi-hit melee combo', frames: 6, fps: 14, loop: false, hint: 'Multi-hit combo: a fast sequence of distinct strikes from different angles (e.g. slash, backslash, thrust), each frame a separate hit, ending in a recovery pose.' },
  { name: 'slash', label: 'Slash', category: 'Combat', action: 'horizontal sword slash', frames: 5, fps: 14, loop: false, hint: 'Sword slash: coil the blade back, sweep it across in a wide horizontal arc at full extension, follow through to the opposite side, recover. Most extreme pose mid-swing.' },
  { name: 'stab', label: 'Stab', category: 'Combat', action: 'forward thrust attack', frames: 4, fps: 14, loop: false, hint: 'Thrust attack: draw the weapon back close to the body, explosive straight forward lunge with full arm and weapon extension, then retract. Peak frame fully extended forward.' },
  { name: 'punch', label: 'Punch', category: 'Combat', action: 'straight punch', frames: 4, fps: 14, loop: false, hint: 'Straight punch: cock the fist back at the hip, drive it forward with shoulder rotation to full extension, retract to guard. Peak frame fully extended.' },
  { name: 'kick', label: 'Kick', category: 'Combat', action: 'high kick', frames: 5, fps: 14, loop: false, hint: 'High kick: plant and chamber the knee, snap the leg out to full extension, hold the impact pose, retract and settle. Peak frame at maximum leg extension.' },
  { name: 'uppercut', label: 'Uppercut', category: 'Combat', action: 'rising uppercut punch', frames: 4, fps: 14, loop: false, hint: 'Uppercut: dip the body low loading the legs, drive upward exploding the fist up through the target, finish with body extended tall. Peak frame reaching upward.' },
  { name: 'block', label: 'Block', category: 'Combat', action: 'raise guard and hold a defensive block', frames: 3, fps: 10, loop: true, hint: 'Defensive block: raise arms or shield to guard, brace with a slight crouch, hold firm with tiny tension shifts. Feet planted, posture steady.' },
  { name: 'parry', label: 'Parry', category: 'Combat', action: 'deflect an incoming attack', frames: 4, fps: 16, loop: false, hint: 'Parry: a sharp deflecting flick of the weapon or arm to one side that knocks an attack away, then snap back to ready. Quick and crisp.' },
  { name: 'dodge', label: 'Dodge', category: 'Combat', action: 'quick sidestep dodge', frames: 4, fps: 16, loop: false, hint: 'Dodge: a fast lean-and-step to one side to evade, body weaving out of the way, then recovering balance. Quick lateral motion.' },
  { name: 'backstep', label: 'Backstep', category: 'Combat', action: 'quick hop backward', frames: 4, fps: 14, loop: false, hint: 'Backstep: a quick defensive hop backward, light push off the front foot, brief airborne drift, land back in guard. Net backward movement.' },
  { name: 'shoot', label: 'Shoot', category: 'Combat', action: 'fire a ranged weapon', frames: 4, fps: 14, loop: false, hint: 'Ranged shot: steady the weapon, fire with a sharp recoil kick pushing the body back, then settle back on target. Show the recoil clearly. No projectile particles separated from the weapon.' },
  { name: 'reload', label: 'Reload', category: 'Combat', action: 'reload a ranged weapon', frames: 5, fps: 10, loop: false, hint: 'Reload sequence: lower the weapon, work the mechanism with the off hand (eject, insert, seat), and raise back to ready. Hands do the distinct work across frames.' },
  { name: 'aim', label: 'Aim', category: 'Combat', action: 'hold a steady aim down sights', frames: 3, fps: 10, loop: true, hint: 'Aiming hold: weapon raised and leveled, body steady and braced, only tiny breathing sway. Posture locked, eyes down the sights.' },
  { name: 'throw', label: 'Throw', category: 'Combat', action: 'throw an object overhand', frames: 5, fps: 12, loop: false, hint: 'Overhand throw: wind the arm back behind the head, whip it forward releasing at full extension, follow through across the body. Peak frame at release.' },
  { name: 'charge-attack', label: 'Charge Attack', category: 'Combat', action: 'charge up then release a powerful attack', frames: 6, fps: 12, loop: false, hint: 'Charged attack: a held loading pose gathering power (body coiled, weapon drawn back), then an explosive release strike at full extension, then recovery. Hold the charge for the first frames.' },
  { name: 'spin-attack', label: 'Spin Attack', category: 'Combat', action: 'spinning 360 attack', frames: 6, fps: 14, loop: false, hint: 'Spin attack: rotate the whole body a full turn while sweeping the weapon around in a wide circle, then settle facing forward. Show distinct rotation angles per frame.' },
  { name: 'guard-break', label: 'Guard Break', category: 'Combat', action: 'stagger backward with guard broken', frames: 4, fps: 12, loop: false, hint: 'Guard break: the raised guard is smashed open, arms fly apart, body rocks backward off balance, briefly exposed. Recoil reads as defense failing.' },
  { name: 'counter', label: 'Counter', category: 'Combat', action: 'absorb a hit then counterattack', frames: 5, fps: 14, loop: false, hint: 'Counter: a tight defensive flinch, then an immediate sharp counterattack exploding forward at full extension, then recovery. Two-beat defense-into-offense.' },
  { name: 'taunt', label: 'Taunt', category: 'Combat', action: 'taunting gesture toward an enemy', frames: 4, fps: 8, loop: true, hint: 'Taunt: a confident provoking gesture — beckoning with a hand, chest puffed, head cocked — looping with attitude. Feet planted, upper body expressive.' },
  { name: 'draw-weapon', label: 'Draw Weapon', category: 'Combat', action: 'draw a weapon and enter ready stance', frames: 5, fps: 10, loop: false, hint: 'Draw weapon: reach for the weapon, pull it free in a sweeping motion, settle into a ready combat stance. Final frame is the ready pose with weapon up.' },
  // ── Magic ──
  { name: 'cast', label: 'Cast', category: 'Magic', action: 'generic spell casting', frames: 5, fps: 12, loop: false, hint: 'Spell casting: arms gather inward in concentration, then thrust forward in a casting pose, followed by recovery. Pose changes only, no floating magical particles.' },
  { name: 'cast-fire', label: 'Cast Fire', category: 'Magic', action: 'cast a fire spell', frames: 6, fps: 12, loop: false, hint: 'Fire spell cast: gather energy at the hands with a coiled stance, then thrust both hands forward releasing the blast. Any flame must be opaque, hard-edged, and touching the hands, not floating particles.' },
  { name: 'cast-ice', label: 'Cast Ice', category: 'Magic', action: 'cast an ice spell', frames: 6, fps: 10, loop: false, hint: 'Ice spell cast: a slow controlled gathering pose, hands sweeping inward, then a sharp pointed release forward. Cold, precise, deliberate motion.' },
  { name: 'cast-lightning', label: 'Cast Lightning', category: 'Magic', action: 'cast a lightning spell', frames: 5, fps: 14, loop: false, hint: 'Lightning cast: a fast raise of the arm overhead charging, then a sharp downward or forward strike releasing the bolt. Quick and snappy. Effects hard-edged and touching the hand only.' },
  { name: 'cast-heal', label: 'Cast Heal', category: 'Magic', action: 'cast a healing spell on self', frames: 5, fps: 8, loop: false, hint: 'Healing cast: bring hands together at the chest in a gentle gathering pose, then open them outward and upward in a soft release, head tilted up. Calm, flowing motion.' },
  { name: 'summon', label: 'Summon', category: 'Magic', action: 'summon by raising arms', frames: 5, fps: 10, loop: false, hint: 'Summon: crouch and gather low, then rise sweeping both arms upward and outward in a grand calling gesture, finishing tall with arms raised. Build to the peak.' },
  { name: 'channel', label: 'Channel', category: 'Magic', action: 'channel energy continuously', frames: 4, fps: 8, loop: true, hint: 'Channeling loop: a sustained focused pose, hands held out gathering energy, body tense with small pulsing shifts and a slight glow at the hands. Looping concentration.' },
  { name: 'buff', label: 'Buff', category: 'Magic', action: 'self power-up buff gesture', frames: 4, fps: 10, loop: false, hint: 'Buff cast: clench fists and pull them inward to the body in a powering-up motion, body tensing and rising slightly, finishing in a strong braced pose.' },
  { name: 'shield-up', label: 'Shield Up', category: 'Magic', action: 'raise a magical shield barrier', frames: 4, fps: 10, loop: false, hint: 'Shield up: sweep one arm forward and out to project a barrier, body braced behind it, then hold. Any barrier shape must be opaque and hard-edged.' },
  { name: 'teleport', label: 'Teleport', category: 'Magic', action: 'vanish and reappear', frames: 5, fps: 14, loop: false, hint: 'Teleport: body compresses and distorts shrinking away to nothing in the first frames, then reforms and expands back into a solid pose. Use silhouette compression, not particle clouds.' },
  { name: 'transform', label: 'Transform', category: 'Magic', action: 'dramatic transformation', frames: 6, fps: 10, loop: false, hint: 'Transformation: a crouched gathering pose, body tensing and shaking, then bursting upward into a new powered-up stance. Build tension then release to a bold final pose.' },
  { name: 'power-up', label: 'Power Up', category: 'Magic', action: 'powering up with surging energy', frames: 5, fps: 10, loop: true, hint: 'Power-up loop: braced wide stance, fists clenched, body trembling with effort and energy surging upward, hair and clothes lifting. Looping intensity, feet planted.' },
  { name: 'meditate', label: 'Meditate', category: 'Magic', action: 'sitting meditation, calm breathing', frames: 4, fps: 4, loop: true, hint: 'Meditation loop: seated cross-legged, hands resting on knees, eyes closed, very slow calm breathing rise and fall. Minimal serene motion.' },
  { name: 'explode', label: 'Explode', category: 'Magic', action: 'release an explosive burst outward', frames: 5, fps: 16, loop: false, hint: 'Explosive release: gather tightly inward, then throw the whole body open releasing a burst outward, then settle. Use the body opening up to imply the blast, effects hard-edged and touching the body.' },
  // ── Damage ──
  { name: 'hurt', label: 'Hurt', category: 'Damage', action: 'recoil from being hit', frames: 3, fps: 10, loop: false, hint: 'Hit reaction: body recoils backward, head snaps back, brief stagger with arms flailing slightly, then a weakened guard pose. Feet roughly in place.' },
  { name: 'hurt-heavy', label: 'Hurt Heavy', category: 'Damage', action: 'stagger hard from a heavy hit', frames: 4, fps: 10, loop: false, hint: 'Heavy hit reaction: the whole body is thrown backward and folds from the impact, head whipping back, nearly losing balance, then a struggling recovery. Bigger than a normal hurt.' },
  { name: 'knockback', label: 'Knockback', category: 'Damage', action: 'knocked backward through the air', frames: 4, fps: 12, loop: false, hint: 'Knockback: launched backward off the feet from a blow, body airborne and tumbling backward, then a hard skidding stop. Clear backward travel through the air.' },
  { name: 'knockdown', label: 'Knockdown', category: 'Damage', action: 'knocked down to the ground', frames: 4, fps: 10, loop: false, hint: 'Knockdown: struck and losing footing, body rotates and drops, landing flat on the back or side on the ground. Final frame fully down.' },
  { name: 'get-up', label: 'Get Up', category: 'Damage', action: 'get up from the ground', frames: 5, fps: 8, loop: false, hint: 'Get up: from lying on the ground, push up with the arms, draw the legs under, rise through a crouch back to standing. Clear upward progression.' },
  { name: 'stun', label: 'Stun', category: 'Damage', action: 'stunned and wobbling in place', frames: 4, fps: 8, loop: true, hint: 'Stunned loop: dazed slumped posture, head lolling, body swaying off balance, knees buckling slightly. Looping wobble, feet barely holding.' },
  { name: 'dizzy', label: 'Dizzy', category: 'Damage', action: 'dizzy with head spinning', frames: 4, fps: 8, loop: true, hint: 'Dizzy loop: head rolling in circles, body wobbling, arms loose and drifting for balance, unfocused. Looping disorientation. No floating star particles.' },
  { name: 'frozen', label: 'Frozen', category: 'Damage', action: 'frozen stiff and trembling', frames: 3, fps: 6, loop: true, hint: 'Frozen loop: body locked rigid mid-pose, arms clamped to the sides, only a tiny brittle tremble. Stiff and immobile, shivering slightly.' },
  { name: 'burning', label: 'Burning', category: 'Damage', action: 'on fire, flinching from flames', frames: 4, fps: 12, loop: true, hint: 'Burning loop: flinching and writhing, patting at the body, hopping in discomfort. Any flame must be opaque, hard-edged, and touching the body, not floating particles.' },
  { name: 'poisoned', label: 'Poisoned', category: 'Damage', action: 'sickened by poison, hunched', frames: 4, fps: 6, loop: true, hint: 'Poisoned loop: hunched and queasy, clutching the stomach, swaying weakly, head drooping. Looping sickly weakness.' },
  { name: 'stagger', label: 'Stagger', category: 'Damage', action: 'stumble and barely keep balance', frames: 4, fps: 10, loop: false, hint: 'Stagger: lurch off balance, arms windmilling to recover, feet shuffling to catch the body, then steadying. Reads as almost falling.' },
  { name: 'death', label: 'Death', category: 'Damage', action: 'stagger, collapse, lie flat on the ground', frames: 5, fps: 8, loop: false, hint: 'Defeat sequence: stagger, collapse to the knees, fall further down, finally lying flat on the ground. Final frame clearly lying down.' },
  { name: 'death-fall', label: 'Death Fall', category: 'Damage', action: 'fall backward and collapse', frames: 4, fps: 8, loop: false, hint: 'Falling death: thrown backward, arms flung out, body arcing back and dropping, landing flat and motionless. Final frame fully down and still.' },
  { name: 'revive', label: 'Revive', category: 'Damage', action: 'rise back to life from the ground', frames: 6, fps: 8, loop: false, hint: 'Revive: from lying flat, the body stirs, lifts, and rises through a kneeling pose back to a strong standing stance, head lifting last. Gradual return of strength.' },
  { name: 'low-hp', label: 'Low Hp', category: 'Damage', action: 'near death, weak and hunched', frames: 4, fps: 6, loop: true, hint: 'Low-HP loop: hunched and exhausted, one hand braced on a knee, heavy labored breathing, slight unsteady sway. Barely standing, looping fatigue.' },
  { name: 'defeat', label: 'Defeat', category: 'Damage', action: 'drop to knees in defeat', frames: 4, fps: 8, loop: false, hint: 'Defeat: shoulders sag, the body sinks down onto the knees, head bowing low in surrender. Ends kneeling and dejected.' },
  // ── Emotion ──
  { name: 'wave', label: 'Wave', category: 'Emotion', action: 'friendly hand wave, body still', frames: 4, fps: 8, loop: true, hint: 'Friendly greeting: one arm raises and waves side to side across frames while the rest of the body stays still. Hand in clearly different positions each frame. Feet planted.' },
  { name: 'cheer', label: 'Cheer', category: 'Emotion', action: 'cheer with arms raised', frames: 4, fps: 10, loop: true, hint: 'Cheering loop: throw both arms up overhead repeatedly with a small hop or bounce, head up, joyful. Energetic looping celebration.' },
  { name: 'clap', label: 'Clap', category: 'Emotion', action: 'clapping hands', frames: 4, fps: 10, loop: true, hint: 'Clapping loop: bring both hands together and apart in front of the chest repeatedly, slight body bounce. Hands clearly open and closed across frames.' },
  { name: 'bow', label: 'Bow', category: 'Emotion', action: 'respectful bow', frames: 4, fps: 8, loop: false, hint: 'Bow: from standing, bend forward at the waist into a respectful bow, hold briefly, then rise back up. Show the full forward bend.' },
  { name: 'nod', label: 'Nod', category: 'Emotion', action: 'nodding the head yes', frames: 3, fps: 8, loop: false, hint: 'Nod: tip the head down and back up in agreement, small body settle. Head clearly moves down then up. Body otherwise still.' },
  { name: 'shake-head', label: 'Shake Head', category: 'Emotion', action: 'shaking the head no', frames: 4, fps: 8, loop: false, hint: 'Head shake: turn the head left and right in refusal, shoulders slightly tense. Head clearly rotates side to side. Body otherwise still.' },
  { name: 'laugh', label: 'Laugh', category: 'Emotion', action: 'laughing happily', frames: 4, fps: 8, loop: true, hint: 'Laughing loop: head tipped back, shoulders bouncing with laughter, maybe a hand to the belly, big smile. Looping bounce of joy.' },
  { name: 'cry', label: 'Cry', category: 'Emotion', action: 'crying sadly', frames: 4, fps: 6, loop: true, hint: 'Crying loop: hands toward the face, shoulders shaking with sobs, head bowed, body hunched. Looping sad tremble. Tears optional but must be small and on the face.' },
  { name: 'angry', label: 'Angry', category: 'Emotion', action: 'furious, fists clenched', frames: 4, fps: 8, loop: true, hint: 'Angry loop: fists clenched, shoulders raised and tense, body trembling with rage, leaning forward, brows down. Looping fury, feet planted.' },
  { name: 'surprised', label: 'Surprised', category: 'Emotion', action: 'startled and recoiling', frames: 3, fps: 12, loop: false, hint: 'Surprise: a sharp startled jolt — body snaps upright and back, arms fly up, head rears, eyes wide. Quick recoil then a frozen shocked pose.' },
  { name: 'think', label: 'Think', category: 'Emotion', action: 'thinking with hand on chin', frames: 4, fps: 6, loop: true, hint: 'Thinking loop: one hand to the chin, head tilted, weight shifting slowly side to side, occasional small head tilt. Pondering, looping subtle motion.' },
  { name: 'point', label: 'Point', category: 'Emotion', action: 'point forward decisively', frames: 4, fps: 10, loop: false, hint: 'Pointing: draw the arm back then thrust it forward extending one finger to point decisively, body leaning into it, then hold. Peak frame fully extended forward.' },
  { name: 'salute', label: 'Salute', category: 'Emotion', action: 'military salute', frames: 4, fps: 8, loop: false, hint: 'Salute: snap one hand up to the brow in a crisp military salute, body straightening to attention, hold, then lower. Sharp and formal.' },
  { name: 'dance', label: 'Dance', category: 'Emotion', action: 'rhythmic dancing', frames: 6, fps: 10, loop: true, hint: 'Dancing loop: rhythmic full-body movement — hips and arms swaying, weight shifting foot to foot, head bobbing to a beat. Distinct fun poses per frame, looping smoothly.' },
  { name: 'victory', label: 'Victory', category: 'Emotion', action: 'victory pose celebration', frames: 4, fps: 8, loop: true, hint: 'Victory loop: a triumphant pose — fist pumped or arms raised, chest out, small confident bounce. Looping celebration, proud and energetic.' },
  { name: 'sad', label: 'Sad', category: 'Emotion', action: 'sad and downcast', frames: 4, fps: 4, loop: true, hint: 'Sad loop: shoulders slumped, head down, arms hanging limp, a slow heavy sway and sigh. Looping melancholy, minimal motion.' },
  { name: 'scared', label: 'Scared', category: 'Emotion', action: 'frightened and cowering', frames: 4, fps: 8, loop: true, hint: 'Scared loop: cowering back, arms raised defensively in front of the face, body trembling and shrinking, knees together. Looping fear.' },
  { name: 'yawn', label: 'Yawn', category: 'Emotion', action: 'yawning sleepily', frames: 4, fps: 6, loop: false, hint: 'Yawn: a big stretch with arms rising and head tilting back, mouth wide in a yawn, then arms lower and shoulders settle. One clear stretch-and-relax.' },
  // ── Interaction ──
  { name: 'pick-up', label: 'Pick Up', category: 'Interaction', action: 'bend down and pick up an item', frames: 4, fps: 10, loop: false, hint: 'Pick up: bend at the knees and waist down toward the ground, close the hand as if grasping an item, then rise back up holding it. Clear down-then-up motion.' },
  { name: 'carry', label: 'Carry', category: 'Interaction', action: 'walk while carrying a load', frames: 6, fps: 8, loop: true, hint: 'Carrying walk loop: walking cycle with both arms held forward or up bearing a load, slightly leaned back to balance the weight, shorter steps. Looping burdened walk.' },
  { name: 'push', label: 'Push', category: 'Interaction', action: 'push a heavy object forward', frames: 6, fps: 8, loop: true, hint: 'Pushing loop: leaning hard forward with both arms extended against an object, legs driving with alternating steps, straining. Looping effortful push.' },
  { name: 'pull', label: 'Pull', category: 'Interaction', action: 'pull a heavy object backward', frames: 6, fps: 8, loop: true, hint: 'Pulling loop: leaning back with both arms drawn in gripping something, legs stepping backward and digging in, straining. Looping effortful pull.' },
  { name: 'open', label: 'Open', category: 'Interaction', action: 'open a door or chest', frames: 4, fps: 10, loop: false, hint: 'Open: reach forward toward a handle, grip and pull or push it open with a turning motion, lean in. Clear reach-and-open action with the arm doing the work.' },
  { name: 'eat', label: 'Eat', category: 'Interaction', action: 'eating food', frames: 4, fps: 8, loop: false, hint: 'Eating: raise a hand to the mouth as if holding food, take a bite with a small head tilt, lower the hand, chew. Clear hand-to-mouth motion.' },
  { name: 'drink', label: 'Drink', category: 'Interaction', action: 'drinking from a cup', frames: 4, fps: 8, loop: false, hint: 'Drinking: raise a hand to the mouth as if holding a cup, tip the head back to drink, then lower. Clear raise-tip-lower motion.' },
  { name: 'read', label: 'Read', category: 'Interaction', action: 'reading a held book', frames: 4, fps: 6, loop: true, hint: 'Reading loop: both hands held out front as if holding an open book, head tilted down scanning, occasional small head shift or page turn. Looping calm study.' },
  { name: 'dig', label: 'Dig', category: 'Interaction', action: 'digging with a shovel', frames: 6, fps: 8, loop: true, hint: 'Digging loop: thrust a shovel down into the ground, scoop, lift and toss the dirt aside, return. Looping dig cycle with clear down-scoop-toss phases.' },
  { name: 'mine', label: 'Mine', category: 'Interaction', action: 'swinging a pickaxe to mine', frames: 6, fps: 10, loop: true, hint: 'Mining loop: raise a pickaxe overhead, swing it down hard into rock with an impact recoil, lift back up. Looping swing cycle with a clear strike frame.' },
  { name: 'chop', label: 'Chop', category: 'Interaction', action: 'chopping with an axe', frames: 6, fps: 10, loop: true, hint: 'Chopping loop: raise an axe up and back, swing it down into a target with an impact jolt, recover up. Looping chop cycle with a clear strike frame.' },
  { name: 'fish', label: 'Fish', category: 'Interaction', action: 'fishing, cast and wait', frames: 5, fps: 6, loop: true, hint: 'Fishing loop: holding a rod out front, a slow gentle bob of the line and small body sway while waiting, occasional tiny tug check. Looping patient wait.' },
]

const presetByName: Map<string, PresetInfo> = new Map(Presets.map((p) => [p.name, p]))

/** Catalog for the picker UI, with the backend-only `hint` stripped. */
export function listPresets(): Omit<PresetInfo, 'hint'>[] {
  return Presets.map(({ hint, ...rest }) => rest)
}

export function presetByNameLookup(name: string): PresetInfo | undefined {
  return presetByName.get(name.trim().toLowerCase())
}

/**
 * motionHint — choreography guide for a state name. 8-direction sets append a
 * direction suffix (e.g. "attack-south-east"); if there's no exact match, strip
 * the suffix and retry on the base keyword.
 */
export function motionHint(stateName: string): string {
  const key = stateName.trim().toLowerCase()
  const direct = presetByName.get(key)
  if (direct) return direct.hint
  const base = stripDirectionSuffix(key)
  if (base !== key) {
    const p = presetByName.get(base)
    if (p) return p.hint
  }
  return ''
}

// Direction keys (compound first, so "attack-south-east" isn't truncated to
// "attack-south"). Kept local to avoid a cycle with direction.ts.
const DIRECTION_KEYS = ['north-west', 'north-east', 'south-west', 'south-east', 'north', 'south', 'east', 'west']

function stripDirectionSuffix(name: string): string {
  for (const d of DIRECTION_KEYS) {
    if (d.includes('-') && name.endsWith('-' + d)) return name.slice(0, -(d.length + 1))
  }
  for (const d of DIRECTION_KEYS) {
    if (!d.includes('-') && name.endsWith('-' + d)) return name.slice(0, -(d.length + 1))
  }
  return name
}
