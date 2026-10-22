// Gate-fixes-2 item 2: an access through a null virtual interface (IEEE
// 1800-2009 25.9, a fatal run-time error) or a null class handle (8.4) stops
// the run at its source position. Nothing after the failed access runs: not
// the rest of its statement (the store or the display), not the statements
// after it, and not the rest of a statement whose called function failed
// (S99-D8). The final procedure runs only after a class handle (S99-D11)
// and shows that `k` was not written. `+case=<n>` selects the access.
interface bus_if;
  logic [7:0] a;
  logic [7:0] mem [0:3];
  function automatic logic [7:0] get();
    return a;
  endfunction
endinterface

class item_c;
  int x;
endclass

module tb;
  bus_if bi();
  virtual bus_if v;
  item_c c;
  int k = 5;
  int which = 0;
  function automatic int read_member();
    return v.a;
  endfunction
  initial begin
    void'($value$plusargs("case=%d", which));
    $display("before");
    case (which)
      0: k = v.a;
      1: $display("display %h", v.a);
      2: k = v.mem[1];
      3: v.mem[1] = 8'h01;
      4: v.a[2] = 1'b1;
      5: k = v.get();
      6: $display("call %0d", read_member());
      7: k = c.x;
      default: $display("display %0d", c.x);
    endcase
    $display("after k=%0d", k);
  end
  final $display("final k=%0d", k);
endmodule
