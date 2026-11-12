// SIM-017: try_get of a record as a case expression is legal (SV 15.4.6,
// 12.5); the copy-out to the record is placed only around a statement that
// evaluates the call once, in a loop condition or step and in an operand that
// may be skipped, so this context is rejected explicitly.
module tb;
  typedef struct {
    int a;
    string s;
  } r_t;
  mailbox #(r_t) m = new();
  r_t r;
  initial begin
    case (m.try_get(r))
      0: $display("empty");
      default: $display("%0d", r.a);
    endcase
    $finish;
  end
endmodule
