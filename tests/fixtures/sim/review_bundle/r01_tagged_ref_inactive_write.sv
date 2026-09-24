// A ref write to the inactive member diagnoses without changing its payload.
module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } tagged_t;
  tagged_t value;
  task automatic set_member(ref tagged_t data);
    data.A = 8'h44;
  endtask
  initial begin
    value = tagged B(8'h55);
    set_member(value);
    $display("AFTER_INACTIVE_REF_WRITE active_B=%h", value.B);
    $finish;
  end
endmodule
