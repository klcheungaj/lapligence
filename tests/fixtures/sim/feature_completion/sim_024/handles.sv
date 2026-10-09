// SIM-024: `%p` of chandles, events, virtual interfaces and process
// handles: null prints `null` (SV 21.2.1.7), other values a fixed word.
interface ifc;
  logic a;
endinterface
module tb;
  ifc i0 ();
  virtual ifc vi;
  chandle ch;
  event ev;
  process p;
  initial begin
    $display("A|%p|%p|%p|%0p|", vi, ch, ev, vi);
    vi = i0;
    p = process::self();
    $display("B|%p|%p|", vi, p);
    $finish(0);
  end
endmodule
