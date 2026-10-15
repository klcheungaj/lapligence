// Decision S45-D8: the gap after a nonconsecutive repetition ends at the
// first sample of the following item. With b7 = (1 => 2 [= 3:6] => 5) and
// the clause's sample sequence, b7 counts once, on the 13th sample: the 5s
// of samples 14 to 18 do not end b7 again, because the gap may not hold the
// 5 of sample 13.
//
// IEEE 1800-2009 19.5.1 (SystemVerilog-1800-2009.txt L32621-32622): "The
// transition following the nonconsecutive repetition may occur after any
// number of sample points so long as the repetition value does not occur
// again." L32730: "Transition bin b7 is incremented on the 13th sample." The
// rule alone would also end b7 on samples 14 to 18; llg follows the
// example. at_least 2 shows the count stays 1.
module tb;
  bit [3:0] v;
  covergroup g;
    once: coverpoint v { bins b7 = (1 => 2 [= 3:6] => 5); }
    twice: coverpoint v { option.at_least = 2; bins b7 = (1 => 2 [= 3:6] => 5); }
  endgroup
  g c = new;
  bit [3:0] values[18] = '{1, 4, 3, 2, 3, 3, 2, 2, 3, 2, 3, 1, 5, 5, 5, 5, 5, 5};
  initial begin
    foreach (values[i]) begin
      v = values[i];
      c.sample();
      if (i == 11 || i == 12 || i == 17)
        $display("sample %0d: %0.2f %0.2f", i + 1, c.once.get_inst_coverage(),
                 c.twice.get_inst_coverage());
    end
    $finish;
  end
endmodule
