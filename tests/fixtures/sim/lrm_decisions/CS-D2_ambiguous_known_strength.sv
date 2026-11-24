// Decision CS-D2: vpi_get_value with vpiStrengthVal on a net bit whose
// value is known but whose strength is a range (a driver with an unknown
// enable beside a weaker driver of the same value) reports the weaker end
// of the range in s0 and the stronger end in s1. A bit with one strength
// reports it in both fields and an unknown bit follows A20-D21.
//
// IEEE 1800-2009 38.15 (SystemVerilog-1800-2009.txt L62675-62676): "If the
// format field in the s_vpi_value structure is set to vpiStrengthVal, the
// value.strength pointer shall point to an array of s_vpi_strengthval
// structures." 28.12.2 L47693: "An ambiguous signal strength can be a range
// of possible values." The text gives no rule for the two fields of a
// known value with a strength range; this is the industry-standard
// behaviour.
module tb;
  reg d, en;
  wire one, low, high;
  // Strong 0 with an unknown enable over a weak 0: 0 from weak to strong.
  bufif1 (low, d, en);
  pulldown (weak0) (low);
  // Strong 1 with an unknown enable under a pull 1: 1 from pull to strong.
  bufif1 (high, ~d, en);
  pullup (pull1) (high);
  // One strength.
  assign (pull1, pull0) one = ~d;
  initial begin
    d  = 1'b0;
    en = 1'bx;
    #1 $display("low=%v high=%v one=%v", low, high, one);
    $d2;
    $finish;
  end
endmodule
