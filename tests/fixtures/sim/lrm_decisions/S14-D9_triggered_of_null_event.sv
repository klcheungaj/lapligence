// Decision S14-D9: `.triggered` of a null event is false, whether the event is
// a variable, a property or an element that an invalid index selects; reading
// it is no null access.
//
// IEEE 1800-2009 15.5.3 (SystemVerilog-1800-2009.txt L20735-20737): "The
//   triggered event property evaluates to true if the given event has been
//   triggered in the current time step and false otherwise. If
//   event_identifier is null, then the triggered event property evaluates to
//   false."
//
// Some simulators report a null access instead.
module tb;
  class holder;
    event e;
  endclass

  event empty = null;
  event several[2];
  holder h;
  int index = 5;

  initial begin
    h = new;
    h.e = null;
    $display("variable %b", empty.triggered);
    $display("property %b", h.e.triggered);
    $display("element %b", several[index].triggered);
    if (empty.triggered) $display("true");
    else $display("false");
    -> several[1];
    $display("event %b", several[1].triggered);
    $finish(0);
  end
endmodule
