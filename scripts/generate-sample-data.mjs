import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const base = fileURLToPath(new URL('../sample-data', import.meta.url))

const surveys = [
  { base: '20250101_AB', full: '20250101_AB_CD', dolphins: ['img_001.jpg', 'img_003.jpg'] },
  { base: '20250102_CD', full: '20250102_CD_EF', dolphins: ['photo_a.jpg'] }
]

const writeJpg = async (path, content) => {
  await writeFile(path, content)
}

const run = async () => {
  await mkdir(base, { recursive: true })
  const rawRoot = join(base, 'Raw')
  const gradedRoot = join(base, 'Graded')
  await mkdir(rawRoot, { recursive: true })
  await mkdir(gradedRoot, { recursive: true })

  for (const survey of surveys) {
    const rawSurvey = join(rawRoot, '2025', '01', survey.base)
    const gradedSurvey = join(gradedRoot, survey.full)
    await mkdir(rawSurvey, { recursive: true })
    await mkdir(gradedSurvey, { recursive: true })

    const rawFiles = ['img_001.jpg', 'img_002.jpg', 'img_003.jpg']
    for (const file of rawFiles) {
      await writeJpg(join(rawSurvey, file), `sample:${file}`)
    }

    for (const file of survey.dolphins) {
      await writeJpg(join(gradedSurvey, file), `sample:${file}`)
    }
  }

  const orphanRaw = join(rawRoot, '2025', '02', '20250103_EF')
  await mkdir(orphanRaw, { recursive: true })
  await writeJpg(join(orphanRaw, 'lonely.jpg'), 'raw:orphan')

  const orphanGraded = join(gradedRoot, '20250104_GH')
  await mkdir(orphanGraded, { recursive: true })
  await writeJpg(join(orphanGraded, 'ghost.jpg'), 'graded:orphan')

  console.log('Sample data created at:', base)
}

run()
