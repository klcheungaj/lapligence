// IEEE 1800-2009 6.24.3 L7534-7537: "If source_t or dest_t contain dynamically
// sized types, then a difference in their sizes will issue an error either at
// compile time or at run time, as soon as it is possible to determine the size
// mismatch." 11.4.14 L15472-15474 instead zero-fills a stream assigned to a
// dynamically sized variable.
// Decision (llg choice): an explicit bit-stream cast to a dynamic array or
// queue type needs a source that is a whole number of its elements; any other
// size is a run-time error. A streaming concatenation assigned without a cast
// still zero-fills (S20-3). Negative case: the run stops with an error after
// the line below.
module tb;
  typedef shortint sd_t[];
  byte q[$];
  sd_t d;
  initial begin
    q = {8'h01, 8'h02, 8'h03, 8'h04};
    d = sd_t'(q);
    $display("whole %0d %h %h", d.size(), d[0], d[1]);
    q.push_back(8'h05);
    d = sd_t'(q);
    $display("unreachable %0d", d.size());
    $finish;
  end
endmodule
