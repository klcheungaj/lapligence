// IEEE 1800-2009 11.4.14 L15472-15474: "If the target represents a dynamically
// sized variable, such as a queue or dynamic array, the variable is resized to
// accommodate the entire stream. If, after resizing, the variable is larger
// than the stream, the stream is left-aligned and zero-filled on the right."
// Decision (LRM text): a stream assigned to a dynamic array or queue gets as
// many elements as hold the whole stream; the last element is zero-filled on
// the right.
module tb;
  shortint w[];
  shortint q[$];
  initial begin
    w = {>>{24'h010203}};
    $display("dynamic %0d %h %h", w.size(), w[0], w[1]);
    q.push_back(16'h7777);
    q = {<<8{24'h010203}};
    $display("queue %0d %h %h", q.size(), q[0], q[1]);
    $finish;
  end
endmodule
