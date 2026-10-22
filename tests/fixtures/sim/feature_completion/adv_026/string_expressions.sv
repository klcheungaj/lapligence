// ADV-026 decision S26-D17: a data argument may be any string expression or an
// integral value holding text; each is read when the task runs.
module tb;
  localparam string IMAGE = "11 22";
  logic [7:0] mem [0:3];
  string image = "33";
  logic [47:0] vector;
  task show(input string tag);
    $display("%s %h %h %h %h", tag, mem[0], mem[1], mem[2], mem[3]);
  endtask
  initial begin
    mem = '{default: 8'h00};
    $sreadmemh(mem, 0, 1, IMAGE);
    show("parameter");
    $sreadmemh(mem, 2, 3, image, "44");
    show("variable");
    image = "@1 55";
    $sreadmemh(mem, 0, 3, image);
    show("changed");
    $sreadmemh(mem, 0, 1, "66", {"7", "7"});
    show("concatenation");
    vector = "88 99";
    $sreadmemh(mem, 2, 3, vector);
    show("vector");
    for (int i = 0; i < 2; i++) begin
      image = $sformatf("@%0d a%0d", i, i);
      $sreadmemh(mem, 0, 3, image);
    end
    show("formatted");
    $finish;
  end
endmodule
