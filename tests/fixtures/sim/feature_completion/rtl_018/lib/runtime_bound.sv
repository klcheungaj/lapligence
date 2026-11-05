// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/lib/runtime_bound.sv
// A cell bound from library rtl whose `%l` conversions are known only at run
// time (T1-D6).
module rtl018_runtime_bound;
  string format = "%l|%L|%m|%%l|%0d";
  string text;
  int value = 7;
  initial begin
    text = $sformatf(format, value);
    $display("%s", text);
    $sformat(text, format, value);
    $display("%s", text);
    $display("%s", $sformatf("%l"));
  end
endmodule
