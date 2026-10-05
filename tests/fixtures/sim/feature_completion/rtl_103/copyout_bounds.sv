// IEEE 1800-2009 11.4.14.4, 13.5: a copy-out `with` range partly outside the
// target array unpacks the in-range elements and reports an error; an unknown
// selector writes nothing. Results go to stderr after the reports.
module tb;
  localparam int STDERR = 32'h8000_0002;
  logic [7:0] arr [0:7];
  logic [3:0] unknown;
  int n;
  task automatic get(output logic [31:0] v);
    v = 32'h11223344;
  endtask
  initial begin
    foreach (arr[k]) arr[k] = 8'h00;
    n = 6;
    get({>>{arr with [n +: 4]}});
    $fdisplay(STDERR, "partial %h %h %h", arr[5], arr[6], arr[7]);
    n = -1;
    get({>>{arr with [n +: 3]}});
    $fdisplay(STDERR, "low %h %h", arr[0], arr[1]);
    unknown = 4'bx;
    get({>>{arr with [unknown +: 2]}});
    $fdisplay(STDERR, "unknown %h %h", arr[0], arr[1]);
    $finish(0);
  end
endmodule
