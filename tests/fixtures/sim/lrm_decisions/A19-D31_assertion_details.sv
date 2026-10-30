// Decision A19-D31: assertion details. A sequence or property instance has
// a location: its start line and column and its end line and end column
// (the column of its last character). Every assertion object, statements
// and instances alike, has vpiInstance and vpiModule: the instance and the
// module that hold it. A property instance has the disable condition its
// declaration writes (NULL without one). A scope iterates the concurrent
// assertions declared directly in it with vpiConcurrentAssertions (a
// labeled assertion is declared in the scope that holds the label); a
// generate scope iterates with vpiAssertion the assertions declared in it
// at any depth. A sequence or property instance has no vpiDefName and no
// coverage counters.
//
// IEEE 1800-2009 37.45 (SystemVerilog-1800-2009.txt L60379-L60400): the
// assertion class (sequence inst, assert, assume, cover, property inst,
// immediate assert, immediate assume, immediate cover) with "instance",
// "-> location str: vpiFile int: vpiStartLine int: vpiColumn int:
// vpiEndLine int: vpiEndColumn"; 37.10 details 2 and 3 (L57924-L57927):
// "vpiModule shall return a module if the object is inside a module
// instance, otherwise it shall return NULL." "vpiInstance shall always
// return the immediate instance (package, module, interface, or program)
// in which the object is instantiated." (assertion is an instance item);
// 37.52 diagram (L60709-L60730): property inst -> vpiDisableCondition ->
// expr; 37.12 diagram (L58033-L58058): scope -> concurrent assertion;
// 37.80 diagram (L61661-L61716): gen scope -> assertion. 40.5.3
// (L66580-L66636) gives coverage counters to "assertion_handle" and 39.3.2
// (L65550-L65568) the module definition to an assertion; the text does not
// say what an instance inside an assertion statement counts, and there llg
// follows industry-standard behaviour, which refuses vpiDefName and the
// counters on sequence and property instances.
module tb;
  logic clk = 0, a = 1, b = 1, rst = 0;
  always #5 clk = ~clk;
  sequence s1(x);
    x ##1 b;
  endsequence
  property p1(x);
    @(posedge clk) disable iff (rst) x |-> a;
  endproperty
  property p2;
    @(posedge clk) a;
  endproperty
  ap1: assert property (p1(a));
  ap2: assert property (p2);
  cs1: cover property (@(posedge clk) s1(a));
  if (1) begin : g
    ga: assert property (@(posedge clk) b);
  end
  initial begin : blk
    #12 $d31;
    $finish;
  end
endmodule
