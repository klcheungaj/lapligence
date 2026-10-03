// IEEE 1800-2009 10.4.2 and 11.4.14.3-11.4.14.4: nonblocking streaming
// assignments to fixed persistent destinations. The source and every `with`
// selector are evaluated at issue; each element update commits in the NBA
// region in issue order. Expected values are independent derivations.
module tb;
  typedef struct { logic [3:0] h; logic [7:0] arr [0:2]; } rec_t;
  localparam int N = 70000;

  logic [7:0] q [0:3];
  logic [7:0] desc [3:0];
  logic [7:0] h8, src8;
  logic [15:0] src16;
  logic [15:0] big [N];
  rec_t r;
  logic [31:0] v32;
  int i;

  initial begin
    q = '{default: 8'h00};
    desc = '{default: 8'h00};
    r.h = 4'h0;
    r.arr = '{default: 8'h00};

    // Selector and source changes after issue do not reach the queued updates.
    i = 1;
    src16 = 16'hA1A2;
    {>>{q with [i +: 2]}} <= src16;
    i = 3;
    src16 = 16'h0000;
    $display("issued %h %h %h %h", q[0], q[1], q[2], q[3]);
    #1;
    $display("committed %h %h %h %h", q[0], q[1], q[2], q[3]);

    // Packed and selected targets in one queued stream, in both directions.
    i = 0;
    {>>{h8, desc with [i +: 2]}} <= 24'hB1B2B3;
    #1;
    $display("mixed %h %h %h %h %h", h8, desc[3], desc[2], desc[1], desc[0]);
    i = 3;
    {<<4{h8, desc with [i -: 2]}} <= 32'h1234_5678;
    #1;
    $display("reversed %h %h %h %h %h", h8, desc[3], desc[2], desc[1], desc[0]);

    // A record member and a descriptor-backed array.
    i = 1;
    {>>{r.h, r.arr with [i +: 2]}} <= 20'h9C1C2;
    i = 69998;
    {>>{big with [i +: 2]}} <= 32'hDEADBEEF;
    i = 0;
    #1;
    $display("member %h %h %h %h", r.h, r.arr[0], r.arr[1], r.arr[2]);
    {>>{r.arr with [0 : 0]}} <= 8'hEE;
    r.arr[0] = 8'h11;
    #1;
    $display("member_static %h %h %h", r.arr[0], r.arr[1], r.arr[2]);
    $display("big %h %h", big[69998], big[69999]);

    // A blocking write before commit is overwritten; later issues win.
    i = 2;
    {>>{q with [i +: 1]}} <= 8'h55;
    q[2] = 8'h66;
    {>>{q with [i -: 2]}} <= 16'h7788;
    $display("before %h %h %h %h", q[0], q[1], q[2], q[3]);
    #1;
    $display("after %h %h %h %h", q[0], q[1], q[2], q[3]);

    // A static range queues ordinary element updates alongside.
    {<<8{q with [0 : 1], src8}} <= 24'hC1C2C3;
    #1;
    $display("static %h %h %h", q[0], q[1], src8);
    $finish(0);
  end
endmodule
