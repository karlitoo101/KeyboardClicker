# Generates a short synthetic mechanical-keyboard click as a 16-bit mono PCM WAV.
# The sound = two quick noise/tone transients (press "thock") with fast decay.
$ErrorActionPreference = "Stop"
$path = "c:\Users\karlm\Downloads\KeyboardClicker\assets\click.wav"
New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null

$sampleRate = 44100
$durationSec = 0.06
$n = [int]($sampleRate * $durationSec)

$rand = New-Object System.Random 12345
$samples = New-Object System.Collections.Generic.List[int16]

for ($i = 0; $i -lt $n; $i++) {
    $t = $i / $sampleRate
    # Fast exponential decay envelope for a sharp click.
    $env = [math]::Exp(-$t * 90.0)
    # Mix: filtered noise + a couple of high-ish tones for a "tick".
    $noise = ($rand.NextDouble() * 2.0 - 1.0)
    $tone1 = [math]::Sin(2 * [math]::PI * 2200 * $t)
    $tone2 = [math]::Sin(2 * [math]::PI * 3300 * $t)
    $v = ($noise * 0.6 + $tone1 * 0.25 + $tone2 * 0.15) * $env
    # A tiny secondary transient near the start for a mechanical "double" feel.
    if ($t -lt 0.008) { $v += ($rand.NextDouble() * 2.0 - 1.0) * 0.4 * [math]::Exp(-$t * 300.0) }
    $s = [int]([math]::Max(-1.0, [math]::Min(1.0, $v)) * 32767 * 0.9)
    $samples.Add([int16]$s)
}

$dataBytes = $samples.Count * 2
$ms = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter($ms)
# RIFF header
$bw.Write([byte[]][char[]]'RIFF')
$bw.Write([int](36 + $dataBytes))
$bw.Write([byte[]][char[]]'WAVE')
# fmt chunk
$bw.Write([byte[]][char[]]'fmt ')
$bw.Write([int]16)          # PCM chunk size
$bw.Write([int16]1)         # PCM format
$bw.Write([int16]1)         # mono
$bw.Write([int]$sampleRate)
$bw.Write([int]($sampleRate * 2)) # byte rate
$bw.Write([int16]2)         # block align
$bw.Write([int16]16)        # bits per sample
# data chunk
$bw.Write([byte[]][char[]]'data')
$bw.Write([int]$dataBytes)
foreach ($s in $samples) { $bw.Write([int16]$s) }
$bw.Flush()
[System.IO.File]::WriteAllBytes($path, $ms.ToArray())
$bw.Dispose(); $ms.Dispose()

"Wrote $path ($((Get-Item $path).Length) bytes, $n samples)" | Out-File "$env:TEMP\genclick.txt"
