// Value passing between productions, IEEE 1800-2009 18.17.7: arguments of
// several types with defaults, typed results read through implicit variables
// (arrays for repeated productions), results that build on nested results,
// and results read in weight code blocks. Only single rules or equal results
// are used, so the output does not depend on the random generator.
module tb;
  typedef enum logic [1:0] {RED, GREEN, BLUE} color_t;
  typedef struct packed {
    logic [7:0] tag;
    logic [15:0] payload;
  } packet_t;

  initial begin
    randsequence (main)
      main : scalars wide arrays nested named_args weighted ;

      scalars : i b r sr s c p bt
        { $display("int %0d byte %0d real %0.3f shortreal %0.2f string %s color %s",
                   i, b, r, sr, s, c.name());
          $display("packet %h bit %b", p, bt); } ;
      int i : { return -42; } ;
      byte b : { return -5; } ;
      real r : { return 2.5 * 1.5; } ;
      shortreal sr : { return 0.25; } ;
      string s : { return "word"; } ;
      color_t c : { return BLUE; } ;
      packet_t p : { return '{tag: 8'hA5, payload: 16'hBEEF}; } ;
      bit bt : { return 1'b1; } ;

      wide : w70(70'h3F_0123_4567_89AB_CDEF) w70 { $display("wide %h %h", w70[1], w70[2]); } ;
      logic [69:0] w70 (logic [69:0] v = 70'h1) : { return v << 1; } ;

      arrays : n(1) n(2) repeat (3) count n(4)
        { $display("n[1]=%0d n[2]=%0d n[3]=%0d count=%0d", n[1], n[2], n[3], count); } ;
      int n (int k) : { return k * 10; } ;
      int count : { static int calls = 0; calls++; return calls; } ;

      nested : sum3 { $display("sum3 %0d", sum3); } label { $display("label %s", label); } ;
      int sum3 : n(1) n(2) n(3) { return n[1] + n[2] + n[3]; } ;
      string label : word("x") word("y") { return {word[1], "-", word[2]}; } ;
      string word (string base, int times = 3) : { string out = ""; repeat (times) out = {out, base}; return out; } ;

      named_args : scaled(.factor(3), .value(4)) scaled(.value(5))
        { $display("scaled %0d %0d", scaled[1], scaled[2]); } ;
      int scaled (int value, int factor = 2) : { return value * factor; } ;

      weighted : choice choice
        { $display("choices %0d %0d", choice[1], choice[2]); } ;
      int choice : one := 1 { return one + 100; } | one := 3 { return one + 100; } ;
      int one : { return 7; } ;
    endsequence
    $finish;
  end
endmodule
