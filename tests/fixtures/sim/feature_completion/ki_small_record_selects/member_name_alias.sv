// A subroutine's own array named like a member array of a module record
// (`cells`) is its own storage: run-time selections of it read and write the
// activation's array, never the record's member (a record with a `string`
// member; `member_name_alias_packed` has one of integral members).
module tb;
  typedef struct { logic [7:0] cells [0:3]; string tag; } native_t;
  native_t nr;

  function automatic int function_sum();
    logic [7:0] cells [0:3];
    int sum = 0;
    foreach (cells[i]) cells[i] = 8'h10 + 8'(i);
    foreach (cells[i]) sum += cells[i];
    return sum;
  endfunction

  task automatic fill(int k);
    logic [7:0] cells [0:3];
    cells[k] = 8'hee;
    $display("task %h", cells[k]);
  endtask

  initial begin
    foreach (nr.cells[i]) nr.cells[i] = 8'h01;
    $display("sum %0d", function_sum());
    fill(2);
    $display("nr %h %h %h %h", nr.cells[0], nr.cells[1], nr.cells[2], nr.cells[3]);
    $finish(0);
  end
endmodule
