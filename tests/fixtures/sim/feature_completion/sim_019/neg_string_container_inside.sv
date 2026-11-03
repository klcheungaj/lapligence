// FIN-001 G3 (F07-11): a queue of strings as an `inside` set item is legal
// (SV 11.4.13) but not supported: string sets traverse fixed and aggregate
// storage only.
module tb;
  string sq[$] = {"a", "b"};
  string s = "b";
  initial begin
    $display("%0d", s inside {sq});
    $finish;
  end
endmodule
