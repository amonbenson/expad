## AI tools in the chats and in the text
Please use "Anthropic Claude Code [4]" as the only reference. This one infers that an Anthropic Model has also been used in the background and references the whole range of Anthropic models, including Claude Opus 5.5, Opus 5, and Sonnet 5.

## Report title
Use "Adapter for Expression Pedals".

## Breadboard parts
The breadboard used an AD7718 ans SN74LVC126A buffers.
- The ADS1241 was an example given in the proposal, but ruled out, because it was way to slow (~15 Hz max, please research the exact value from the datasheet).
- The 74HC125-type buffers were also proposed, but ruled out because they weren't available in the required package
- The breadboard capacitance is typically in the 10-20 pF range, as confirmed by online sources, so that measurement must have been caused by something else. Do not include that nF value, because it makes no sense.

## Reason for the AD7718
Please research the datasheets on your own again to gather empirical proof for the choice of the AD7718, in terms of operating voltages, speed, resolution, package, and availability. The ADS8688 was a bit too overkill for the application. The AD7124-8 was not available in a suitable package. The ADS1241 was too slow as stated in the previous question. The price was also a factor, but not as important.

## Reason for one ground net
The main reason for dropping the star point was that the PCB should be as small as possible, and only 2-Layer. A separate ground net would have required much larger spacing between the components. Also, due to the way digital and analog signals are interconnected at the analog switches and need to cross each other, this would have further increased the board size, potentially leading to other issues (reflections on the SPI lines, etc.). Please confirm this claim from the schematic netlist and the final PCB layout.

## Reason for the filter values
The arguments given in the box should be correct, but each needs to be researched and confirmed by independent sources again.

## Supply voltage and polarity
The LM74700 was dropped, because it was meant for reverse battery protection specifically, and overkill for the application and its voltage ranges (citation needed/datasheet). The final circuit was chosen to be as minimal as possible (so the rectifier bridge was dropped) to work with common Pedal Board voltages (9V, but should also handle 12V). That's also the reason why the DC Jack is center-negative. On a later iteration, the rectifier might be re-added to support both polarities, but care must be taken to ensure no additional noise is introduced and that's also a reason why it was dropped from this first prototype, to not further complicate the design and introduce issues.

## Choice of 1 kOhm
1k was chosen with a few calculations based on that table (10k pedal and 500k pedal). The full calculation series described in that table was created later on.

## Choice of 47 kOhm
Please do some more research to this until you find the source that confirms the choice of 47 Ohm. Ideally use a PDF guide document by some big manufacturer (Texas Instruments, Analog Devices, etc.). For example, this forum post talks about using larger resistor values for lower speed clocks: https://electronics.stackexchange.com/questions/336797/calculating-series-resistors-for-i2c-spi-lines-to-reduce-emi-and-ringing

## LED limit
The limit in the final firmware was set due to the visual brightness. The prototype board exposes the bare PCB without any diffusers on the LEDs, so the full brightness at 10/255 is already very bright.

## Board facts
The board outline is 88.65mm x 69.85mm. It was ordered on Aisler. As it was ordered by the supervisor, I don't know the exact date of order and arrival. Please leave out those dates out of the report, they are not that relevant.

## Update-rate target
The target of 50 to 100 Hz was proposed during Q&A session after the first milestone presentation. This can probably be roughly confirmed by how fast humans can move their feet on a pedal. Please do some research to this, and find a good source that roughly confirms this frequency, but don't spend too much time on proving this.

## Android tablet drops
The captive portal helped, but connection drops still happened from time to time. At this point, you can leave this as an open issue/improvement to the firmware that can be further investigated in future work.

## Supply voltage of the measurements
The supply voltage was set to 9 V with a 1 A current limit. Probe: PVP2350, 10X, compensated

## Self-test log
Here is the self-test log:
```
[INFO ] Initializing pull switches (detect_pin_mapping src/bin/detect_pin_mapping.rs:115)
[INFO ] Initializing ADCs (detect_pin_mapping src/bin/detect_pin_mapping.rs:125)
[INFO ] ADC full scale: 2.56V (detect_pin_mapping src/bin/detect_pin_mapping.rs:140)
[INFO ] ADC 0: ground input 0.0V, reference input 2.4999998V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:92)
[INFO ] ADC 1: ground input 0.0V, reference input 2.4999995V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:92)
[INFO ] Switching every contact to both rails - unplug all jacks for a clean result (detect_pin_mapping src/bin/detect_pin_mapping.rs:144)
[INFO ] Jack 0 Tip (switch chip 3 tap 0) -> ADC 1 channel 2: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on TipSwitch (ADC 1 channel 4) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Jack 0 Ring (switch chip 3 tap 1) -> ADC 1 channel 8: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 0 Sleeve (switch chip 3 tap 2) -> ADC 1 channel 3: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 0 TipSwitch (switch chip 3 tap 3) -> ADC 1 channel 4: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on Tip (ADC 1 channel 2) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Jack 1 Tip (switch chip 2 tap 0) -> ADC 1 channel 7: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on TipSwitch (ADC 1 channel 1) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Jack 1 Ring (switch chip 2 tap 1) -> ADC 1 channel 0: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 1 Sleeve (switch chip 2 tap 2) -> ADC 1 channel 6: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 1 TipSwitch (switch chip 2 tap 3) -> ADC 1 channel 1: 2.4999995V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on Tip (ADC 1 channel 7) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Jack 2 Tip (switch chip 1 tap 0) -> ADC 0 channel 2: 2.4961267V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on TipSwitch (ADC 0 channel 4) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Jack 2 Ring (switch chip 1 tap 1) -> ADC 0 channel 8: 2.4999998V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 2 Sleeve (switch chip 1 tap 2) -> ADC 0 channel 3: 2.4999998V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 2 TipSwitch (switch chip 1 tap 3) -> ADC 0 channel 4: 2.4999998V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on Tip (ADC 0 channel 2) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Jack 3 Tip (switch chip 0 tap 0) -> ADC 0 channel 7: 2.4961267V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on TipSwitch (ADC 0 channel 1) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Jack 3 Ring (switch chip 0 tap 1) -> ADC 0 channel 0: 2.4999998V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 3 Sleeve (switch chip 0 tap 2) -> ADC 0 channel 6: 2.4999998V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ] Jack 3 TipSwitch (switch chip 0 tap 3) -> ADC 0 channel 1: 2.4999998V / 0.0V - OK (detect_pin_mapping src/bin/detect_pin_mapping.rs:169)
[INFO ]     also follows on Tip (ADC 0 channel 7) - normalled, no plug inserted (detect_pin_mapping src/bin/detect_pin_mapping.rs:210)
[INFO ] Checked every contact in 1019ms, both ADCs converting in parallel (detect_pin_mapping src/bin/detect_pin_mapping.rs:234)
```

## Supply data quality
As the measurement is close to the noise floor, the exact value of the measurement is not that meaningful anyway. You can just blindly ignore the slightly worse reading. I will come back and mark that manually later, but for now, just leave it as it is with no marking/annotation.

Yes, the noise floor was captured with the probe tip connected to ground. The exact cause for the 5 V spike is not known, but you can add the switching regulator as a potential suspected cause.

## Accuracy and linearity measurement
The measurement with precision resistors was not done due to timing constraints. The Ohmmeter gives the following totals:
- 11.39 kOhm for the cheaper Expression pedal (M-Audio EX-P)
- 9.81 kOhm for the more expensive Expression pedal (Boss EV-30)
- 8.7 Ohm for the closed footswitch (Boss FS-5U)

## Settling series
The settling series was, again, not done due to timing constraints. This was therefore a deliberate choice of prioritizing. Please present the data you have.

## Rate with several pedals
The rate with two pedals was measured to be around the expected value. You can just write that, the exact measurements don't exist anymore. Four pedals were not available, but it was tested with two expression pedals, one footswitch, and one disconnected cable.

## End-to-end latency
The end-to-end latency was not measured, because the pedal-to-usb latence, which is the only latency this project can influence, is already a very small fraction of the total latency. In other words, the majority of the latency comes from the USB stack on the host computer and the DAW software used. You can report some default/theoretically computed latency for the firmware stack and say that the latency of the adapter will be around that range and therefore negligible compared to the total latency of the whole system.

## Pedal models and coverage
The pedal models are named in a previos open question (Accuracy and linearity measurement). A rheostat was not available for testing, but was tested with a bare 100k potentiometer. Multiple sustain pedals were tested, they showed the same behavior as the Boss FS-5U footswitch. A plug-cycle test was not done, but during testing pedals were occasionally mis-detected, but then the firmware was able to recover automatically after a few seconds (most common error was a footswitch beeing detected as a rheostat when it was plugged in too slowly or the jack was moved/rotated during operation).
