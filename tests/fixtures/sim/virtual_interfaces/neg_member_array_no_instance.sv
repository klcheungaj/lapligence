// No instance of `ifc` exists, so `v` can only be null and the use could
// only fail at run time (25.9 L44894). A view of the unpacked member takes
// its shape from an instance's storage, which there is none of, so the use
// is rejected where it is written (docs/known_issues.md).
interface ifc;
  int a[4];
  logic [3:0] p;
endinterface

module tb;
  virtual ifc v;
  int n;
  initial begin
    n = (v == null) ? 1 : v.p;
    #1 v.a[1] = 3;
  end
endmodule
