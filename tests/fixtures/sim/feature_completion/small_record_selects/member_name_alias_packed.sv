// A subroutine's own array named like a member array of a module record
// (`cells`) is its own storage: run-time selections of it read and write the
// activation's array, never the record's member (a record of integral
// members held as one signal per leaf; `member_name_alias` has a `string`
// member).
module tb;
  typedef struct { logic [7:0] cells [0:3]; int tag; } packed_t;
  packed_t pr;

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
    foreach (pr.cells[i]) pr.cells[i] = 8'h02;
    $display("sum %0d", function_sum());
    fill(2);
    $display("pr %h %h %h %h", pr.cells[0], pr.cells[1], pr.cells[2], pr.cells[3]);
    $finish(0);
  end
endmodule
