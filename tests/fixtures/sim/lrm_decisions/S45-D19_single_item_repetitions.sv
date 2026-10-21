// Decision S45-D19: a transition set of one range list with a goto
// (`[-> n]`) or nonconsecutive (`[= n]`) repetition, or a consecutive range
// (`[* m:n]`) that admits more than one repetition, is a legal transition bin
// even when the repetition can be 1. One sample of 3 completes (3 [-> 1]),
// (3 [-> 1:2]), (3 [= 1]), (3 [= 1:2]) and (3 [* 1:3]); a bin counts at most
// once per sample. Only a single range list alone or with a consecutive
// repetition that evaluates to 1, (3) or (3 [* 1]), is length 0 (S45-D11).
// The samples 1, 3, 3, 2, 3 count every bin 0, 1, 2, 2, 3 times; (3 [* 2])
// counts 0, 0, 1, 1, 1 as a control.
//
// IEEE 1800-2009 19.5.1 (SystemVerilog-1800-2009.txt L32735-32737):
// "Transition bin specifications of length 0 shall be illegal. These are
// transition bin specifications containing a trans_set production of a
// single value_range, e.g., (0) or ([0:1]), or a single value_range with a
// repeat_range evaluating to 1, e.g., (0[*1]) or ([0:1][*1])." The text does
// not name goto or nonconsecutive repetitions or a repeat_range that can
// exceed 1. L32602-32606: "3 [-> 3] is the same as ...=>3...=>3...=>3";
// L32626-32630: "3 [= 2] is same as ...=>3...=>3". L32732: "A
// transition bin is incremented at most once per sample."
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289).
module tb;
  bit [3:0] v;
  covergroup g;
    option.per_instance = 1;
    goto1_1: coverpoint v { bins b = (3 [-> 1]); }
    goto1_3: coverpoint v { option.at_least = 3; bins b = (3 [-> 1]); }
    goto12_2: coverpoint v { option.at_least = 2; bins b = (3 [-> 1:2]); }
    goto12_3: coverpoint v { option.at_least = 3; bins b = (3 [-> 1:2]); }
    non1_2: coverpoint v { option.at_least = 2; bins b = (3 [= 1]); }
    non12_3: coverpoint v { option.at_least = 3; bins b = (3 [= 1:2]); }
    con13_1: coverpoint v { bins b = (3 [* 1:3]); }
    con13_3: coverpoint v { option.at_least = 3; bins b = (3 [* 1:3]); }
    con2_1: coverpoint v { bins b = (3 [* 2]); }
    con2_2: coverpoint v { option.at_least = 2; bins b = (3 [* 2]); }
  endgroup
  g c = new;
  bit [3:0] values[5] = '{1, 3, 3, 2, 3};
  initial begin
    foreach (values[i]) begin
      v = values[i];
      c.sample();
      $display("sample %0d v=%0d: %0.0f %0.0f %0.0f %0.0f %0.0f %0.0f %0.0f %0.0f %0.0f %0.0f",
               i + 1, v, c.goto1_1.get_inst_coverage(), c.goto1_3.get_inst_coverage(),
               c.goto12_2.get_inst_coverage(), c.goto12_3.get_inst_coverage(),
               c.non1_2.get_inst_coverage(), c.non12_3.get_inst_coverage(),
               c.con13_1.get_inst_coverage(), c.con13_3.get_inst_coverage(),
               c.con2_1.get_inst_coverage(), c.con2_2.get_inst_coverage());
    end
    $finish;
  end
endmodule
