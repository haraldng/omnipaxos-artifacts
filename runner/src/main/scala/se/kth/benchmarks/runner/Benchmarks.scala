package se.kth.benchmarks.runner

import kompics.benchmarks.benchmarks._
import kompics.benchmarks.messages._
import scala.concurrent.Future
import scala.util.{Failure, Success, Try}
import com.lkroll.common.macros.Macros
import scala.concurrent.duration._

case class BenchmarkRun[Params](name: String, symbol: String, invoke: (Runner.Stub, Params) => Future[TestResult]);

trait Benchmark {
  def name: String;
  def symbol: String;
  def withStub(stub: Runner.Stub, testing: Boolean)(f: (Future[TestResult], ParameterDescription, Long) => Unit): Unit;
  def requiredRuns(testing: Boolean): Long;
}
object Benchmark {
  def apply[Params](b: BenchmarkRun[Params],
                    space: ParameterSpace[Params],
                    testSpace: ParameterSpace[Params]): BenchmarkWithSpace[Params] =
    BenchmarkWithSpace(b, space, testSpace);
  def apply[Params](name: String,
                    symbol: String,
                    invoke: (Runner.Stub, Params) => Future[TestResult],
                    space: ParameterSpace[Params],
                    testSpace: ParameterSpace[Params]): BenchmarkWithSpace[Params] =
    BenchmarkWithSpace(BenchmarkRun(name, symbol, invoke), space, testSpace);
}
case class BenchmarkWithSpace[Params](b: BenchmarkRun[Params],
                                      space: ParameterSpace[Params],
                                      testSpace: ParameterSpace[Params])
    extends Benchmark {
  override def name: String = b.name;
  override def symbol: String = b.symbol;
  def run = b.invoke;
  override def withStub(stub: Runner.Stub,
                        testing: Boolean)(f: (Future[TestResult], ParameterDescription, Long) => Unit): Unit = {
    var index = 0L;
    val useSpace = if (testing) testSpace else space;
    useSpace.foreach { p =>
      index += 1L;
      f(run(stub, p), useSpace.describe(p), index)
    }
  }
  override def requiredRuns(testing: Boolean): Long = if (testing) testSpace.size else space.size;
}

object Benchmarks extends ParameterDescriptionImplicits {

  implicit class ExtLong(i: Long) {
    def mio: Long = i * 1000000L;
    def k: Long = i * 1000L;
  }

  //implicit def seq2param[T: ParameterDescriptor](s: Seq[T]): ParameterSpace[T] = ParametersSparse1D(s);

  val pingPong = Benchmark(
    name = "Ping Pong",
    symbol = "PINGPONG",
    invoke = (stub, request: PingPongRequest) => {
      stub.pingPong(request)
    },
    space = ParameterSpacePB
      .mapped(1L.mio to 10L.mio by 1L.mio)
      .msg[PingPongRequest](n => PingPongRequest(numberOfMessages = n)),
    testSpace =
      ParameterSpacePB.mapped(10L.k to 100L.k by 10L.k).msg[PingPongRequest](n => PingPongRequest(numberOfMessages = n))
  );

  val netPingPong = Benchmark(
    name = "Net Ping Pong",
    symbol = "NETPINGPONG",
    invoke = (stub, request: PingPongRequest) => {
      stub.netPingPong(request)
    },
    space =
      ParameterSpacePB.mapped(1L.k to 10L.k by 1L.k).msg[PingPongRequest](n => PingPongRequest(numberOfMessages = n)),
    testSpace =
      ParameterSpacePB.mapped(100L to 1L.k by 100L).msg[PingPongRequest](n => PingPongRequest(numberOfMessages = n))
  );

  val throughputPingPong = Benchmark(
    name = "Throughput Ping Pong",
    symbol = "TPPINGPONG",
    invoke = (stub, request: ThroughputPingPongRequest) => {
      stub.throughputPingPong(request)
    },
    space = ParameterSpacePB
      .cross(List(1L.mio, 10L.mio), List(10, 50, 500), List(1, 2, 4, 8, 16, 24, 32, 34, 36, 38, 40), List(true, false))
      .msg[ThroughputPingPongRequest] {
        case (n, p, par, s) =>
          ThroughputPingPongRequest(messagesPerPair = n, pipelineSize = p, parallelism = par, staticOnly = s)
      },
    testSpace = ParameterSpacePB
      .cross(10L.k to 100L.k by 30L.k, List(10, 500), List(1, 4, 8), List(true, false))
      .msg[ThroughputPingPongRequest] {
        case (n, p, par, s) =>
          ThroughputPingPongRequest(messagesPerPair = n, pipelineSize = p, parallelism = par, staticOnly = s)
      }
  );

  val netThroughputPingPong = Benchmark(
    name = "Net Throughput Ping Pong",
    symbol = "NETTPPINGPONG",
    invoke = (stub, request: ThroughputPingPongRequest) => {
      stub.netThroughputPingPong(request)
    },
    space = ParameterSpacePB
      .cross(List(1L.k, 10L.k, 20L.k), List(10, 100, 1000), List(1, 2, 4, 8, 16, 24), List(true, false))
      .msg[ThroughputPingPongRequest] {
        case (n, p, par, s) =>
          ThroughputPingPongRequest(messagesPerPair = n, pipelineSize = p, parallelism = par, staticOnly = s)
      },
    testSpace = ParameterSpacePB
      .cross(100L to 1L.k by 300L, List(10, 100, 1000), List(1, 4, 8), List(true, false))
      .msg[ThroughputPingPongRequest] {
        case (n, p, par, s) =>
          ThroughputPingPongRequest(messagesPerPair = n, pipelineSize = p, parallelism = par, staticOnly = s)
      }
  );

  private val windowDataSize = 0.008; // 8kB in MB
  private val windowLengthUtil = utils.Conversions.SizeToTime(windowDataSize, 1.millisecond); // 8kB/s in MB
  private val windowLength = windowLengthUtil.timeForMB(windowDataSize); // 1s window
  val streamingWindows = Benchmark(
    name = "Streaming Windows",
    symbol = "STREAMINGWINDOWS",
    invoke = (stub, request: StreamingWindowsRequest) => {
      stub.streamingWindows(request)
    },
    space = ParameterSpacePB
      .cross(List(1, 2, 4, 8, 16), List(100, 1000), List(0.01, 1.0, 100.0), List(10))
      .msg[StreamingWindowsRequest] {
        case (np, bs, ws, nw) => {
          val windowAmp = (ws / windowDataSize).round;
          StreamingWindowsRequest(numberOfPartitions = np,
                                  batchSize = bs,
                                  windowSize = windowLength,
                                  numberOfWindows = nw,
                                  windowSizeAmplification = windowAmp)
        }
      },
    testSpace = ParameterSpacePB
      .cross(List(1, 2), List(100), List(0.01, 0.1, 1.0, 10.0), List(10))
      .msg[StreamingWindowsRequest] {
        case (np, bs, ws, nw) => {
          val windowAmp = (ws / windowDataSize).round;
          StreamingWindowsRequest(numberOfPartitions = np,
                                  batchSize = bs,
                                  windowSize = windowLength,
                                  numberOfWindows = nw,
                                  windowSizeAmplification = windowAmp)
        }
      }
  );

  val atomicRegister = Benchmark(
    name = "Atomic Register",
    symbol = "ATOMICREGISTER",
    invoke = (stub, request: AtomicRegisterRequest) => {
      stub.atomicRegister(request)
    },
    space = ParameterSpacePB
      .cross(List((0.5f, 0.5f), (0.95f, 0.05f)), List(3, 5, 7, 9), List(10L.k, 20L.k, 40L.k, 80L.k))
      .msg[AtomicRegisterRequest] {
        case ((read_workload, write_workload), p, k) =>
          AtomicRegisterRequest(readWorkload = read_workload,
                                writeWorkload = write_workload,
                                partitionSize = p,
                                numberOfKeys = k)
      },
    testSpace = ParameterSpacePB
      .cross(List((0.5f, 0.5f)), List(3), List(1000))
      .msg[AtomicRegisterRequest] {
        case ((rwl, wwl), p, k) =>
          AtomicRegisterRequest(readWorkload = rwl, writeWorkload = wwl, partitionSize = p, numberOfKeys = k)
      }
  );

  val fibonacci = Benchmark(
    name = "Fibonacci",
    symbol = "FIBONACCI",
    invoke = (stub, request: FibonacciRequest) => {
      stub.fibonacci(request)
    },
    space = ParameterSpacePB
      .mapped(26 to 32 by 1)
      .msg[FibonacciRequest](n => FibonacciRequest(fibNumber = n)),
    testSpace = ParameterSpacePB
      .mapped(22 to 28 by 1)
      .msg[FibonacciRequest](n => FibonacciRequest(fibNumber = n))
  );

  val chameneos = Benchmark(
    name = "Chameneos",
    symbol = "CHAMENEOS",
    invoke = (stub, request: ChameneosRequest) => {
      stub.chameneos(request)
    },
    space = ParameterSpacePB
      .cross(List(2, 3, 4, 5, 6, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 128), List(2.mio))
      .msg[ChameneosRequest] {
        case (nc, nm) => ChameneosRequest(numberOfChameneos = nc, numberOfMeetings = nm)
      },
    testSpace = ParameterSpacePB
      .cross(List(2, 3, 4, 5, 6, 7, 8, 16), List(100.k))
      .msg[ChameneosRequest] {
        case (nc, nm) => ChameneosRequest(numberOfChameneos = nc, numberOfMeetings = nm)
      }
  );

  val allPairsShortestPath = Benchmark(
    name = "All-Pairs Shortest Path",
    symbol = "APSP",
    invoke = (stub, request: APSPRequest) => {
      stub.allPairsShortestPath(request)
    },
    space = ParameterSpacePB
      .cross(List(128, 256, 512, 1024), List(16, 32, 64))
      .msg[APSPRequest] {
        case (nn, bs) => {
          assert(nn % bs == 0, "BlockSize must evenly divide nodes!");
          APSPRequest(numberOfNodes = nn, blockSize = bs)
        }
      },
    testSpace = ParameterSpacePB
      .cross(List(128, 192, 256), List(16, 32, 64))
      .msg[APSPRequest] {
        case (nn, bs) => {
          assert(nn % bs == 0, "BlockSize must evenly divide nodes!");
          APSPRequest(numberOfNodes = nn, blockSize = bs)
        }
      }
  );

  /*** split into different parameter spaces as some parameters are dependent on each other ***/
  private val atomicBroadcastTestNodes = List(3);
  private val atomicBroadcastTestDuration= List(1*60);
  private val atomicBroadcastTestConcurrentProposals = List(200L);

  private val atomicBroadcastNodes = List(3);
  private val atomicBroadcastDuration = List(5*60);
  private val atomicBroadcastConcurrentProposals = List(500L, 5L.k, 50L.k);

  private val algorithms = List("vr", "multi-paxos", "paxos", "raft", "raft_pv_qc");
  private val reconfig = List("single", "majority");
  private val reconfig_policy = List("replace-follower", "replace-leader");
  private val network_scenarios = List("fully_connected", "quorum_loss-60", "constrained_election-60", "chained-60")
  private val election_timeout_ms = List(10L.k)

  private val atomicBroadcastNormalTestSpace = ParameterSpacePB // test space without reconfig
    .cross(
      algorithms,
      atomicBroadcastTestNodes,
      atomicBroadcastTestDuration,
      atomicBroadcastTestConcurrentProposals,
      List("off"),
      List("none"),
      network_scenarios,
      election_timeout_ms
    );

  private val atomicBroadcastReconfigTestSpace = ParameterSpacePB // test space with reconfig
    .cross(
      algorithms,
      atomicBroadcastTestNodes,
      atomicBroadcastTestDuration,
      atomicBroadcastTestConcurrentProposals,
      reconfig,
      reconfig_policy,
      List("fully_connected"),
      election_timeout_ms
    );

  private val atomicBroadcastTestSpace = atomicBroadcastNormalTestSpace.append(atomicBroadcastReconfigTestSpace);

  private val atomicBroadcastNormalSpace = ParameterSpacePB
    .cross(
      List("paxos", "raft", "raft_pv_qc", "vr"),
      atomicBroadcastNodes,
      atomicBroadcastDuration,
      List(500L),
      List("off"),
      List("none"),
      List("chained-60", "chained-120", "chained-240"),
      List(5L, 50L, 500L, 5L.k)
    );

  private val multiPaxosNormalSpace = ParameterSpacePB
    .cross(
      List("multi-paxos"),
      atomicBroadcastNodes,
      atomicBroadcastDuration,
      atomicBroadcastConcurrentProposals,
      List("off"),
      List("none"),
      List("fully_connected"),
      election_timeout_ms
    );

  private val atomicBroadcastReconfigSpace = ParameterSpacePB
    .cross(
      algorithms,
      atomicBroadcastNodes,
      atomicBroadcastDuration,
      atomicBroadcastConcurrentProposals,
      reconfig,
      reconfig_policy,
      network_scenarios,
      election_timeout_ms
    );

  private val atomicBroadcastSpace = multiPaxosNormalSpace.append(atomicBroadcastNormalSpace);

  private val latencySpace = ParameterSpacePB
    .cross(
      algorithms,
      List(3),
      List(1L.k),
      List(1L),
      List("off"),
      List("none"),
      network_scenarios,
      election_timeout_ms
    );

  val atomicBroadcast = Benchmark(
    name = "Atomic Broadcast",
    symbol = "ATOMICBROADCAST",
    invoke = (stub, request: AtomicBroadcastRequest) => {
      stub.atomicBroadcast(request)
    },
    space = atomicBroadcastSpace
      .msg[AtomicBroadcastRequest] {
        case (a, nn, d, cp, r, rp, ns, et) =>
          AtomicBroadcastRequest(
            algorithm = a,
            numberOfNodes = nn,
            durationSecs = d,
            concurrentProposals = cp,
            reconfiguration = r,
            reconfigPolicy = rp,
            networkScenario = ns,
            electionTimeoutMs = et,
          )
      },
    testSpace = atomicBroadcastNormalTestSpace
      .msg[AtomicBroadcastRequest] {
        case (a, nn, d, cp, r, rp, ns, et) =>
          AtomicBroadcastRequest(
            algorithm = a,
            numberOfNodes = nn,
            durationSecs = d,
            concurrentProposals = cp,
            reconfiguration = r,
            reconfigPolicy = rp,
            networkScenario = ns,
            electionTimeoutMs = et,
          )
      }
  );

  val benchmarks: List[Benchmark] = Macros.memberList[Benchmark];
  lazy val benchmarkLookup: Map[String, Benchmark] = benchmarks.map(b => (b.symbol -> b)).toMap;
}
