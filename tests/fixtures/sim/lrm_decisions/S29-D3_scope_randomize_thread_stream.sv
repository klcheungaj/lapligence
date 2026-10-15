// IEEE 1800-2009 18.12 L30918: "The scope randomize function behaves exactly
// the same as a class randomize method, except that it operates on the
// variables of the current scope instead of class member variables."
// 18.14 L31136-31139 lists $urandom, srandom() and randomize() among the
// calls that use an RNG; 18.15 L31270: "Each object maintains its own
// internal RNG, which is used exclusively by its randomize() method."
// Decision (llg choice; the text names no stream for std::randomize): scope
// randomize draws from the calling thread's RNG. Reseeding the thread
// repeats its values, and a call advances the thread's stream, so the next
// $urandom differs from the value it would have returned without the call.
module tb;
  initial begin
    process p;
    bit [15:0] x, first, again;
    int unsigned u_with, u_without;
    p = process::self();
    p.srandom(7);
    void'(std::randomize(x) with { x > 16'd10; });
    first = x;
    u_with = $urandom;
    p.srandom(7);
    void'(std::randomize(x) with { x > 16'd10; });
    again = x;
    p.srandom(7);
    u_without = $urandom;
    $display("repeats=%0d advances=%0d", first == again, u_with != u_without);
    $finish;
  end
endmodule
