// A string `ref` actual whose selector is invalid binds a reference that reads
// the default and ignores writes (SV 7.4.6); the run continues.
module tb;
  string fs[3];
  string qs[$];
  string ds[];
  int k = 7;
  task automatic tr(ref string x);
    $display("read '%s'", x);
    x = "lost";
    $display("kept '%s'", x);
  endtask
  initial begin
    fs[1] = "a";
    tr(fs[k]);
    tr(qs[k]);
    tr(ds[k]);
    $display("%0d %0d '%s'", qs.size(), ds.size(), fs[1]);
    $finish;
  end
endmodule
