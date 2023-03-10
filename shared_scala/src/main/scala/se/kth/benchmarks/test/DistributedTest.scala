package se.kth.benchmarks.test

import org.scalatest._
import scala.util.{Failure, Success, Try}
import scala.concurrent.{Await, Future}
import scala.concurrent.duration._
import kompics.benchmarks.benchmarks._
import kompics.benchmarks.messages._
import kompics.benchmarks.distributed._
import io.grpc.{ManagedChannelBuilder, Server, ServerBuilder}
import se.kth.benchmarks._
import com.typesafe.scalalogging.StrictLogging

class DistributedTest(val benchFactory: BenchmarkFactory) extends Matchers with StrictLogging {

  private var implemented: List[String] = Nil;
  private var notImplemented: List[String] = Nil;

  private var allowedFailures: List[String] = List("RSE");

  val timeout = 5.minutes; // some of these tests can take a long time

  def testFail(): Unit = {
    allowedFailures ::= "Benchmark was purposefully failed at stage";
    this.test();
    allowedFailures = allowedFailures.tail;
  }

  def test(): Unit = {
    val runnerPort = 45678;
    val runnerAddrS = s"127.0.0.1:$runnerPort";
    val masterPort = 45679;
    val masterAddrS = s"127.0.0.1:$masterPort";
    val clientPorts = Array(45680, 45681, 45682, 45683);
    val clientAddrs = clientPorts.map(clientPort => s"127.0.0.1:$clientPort");

    val numClients = 4;

    /*
     * Setup
     */

    val masterThread = new Thread("BenchmarkMaster") {
      override def run(): Unit = {
        logger.debug("Starting master");
        val runnerAddr = Util.argToAddr(runnerAddrS).get;
        val masterAddr = Util.argToAddr(masterAddrS).get;
        BenchmarkMaster.run(numClients, masterAddr.port, runnerAddr.port, benchFactory);
        logger.debug("Finished master");
      }
    };
    masterThread.start();

    val clientThreads = clientAddrs.map { clientAddrS =>
      val clientThread = new Thread(s"BenchmarkClient-$clientAddrS") {
        override def run(): Unit = {
          logger.debug(s"Starting client $clientAddrS");
          val masterAddr = Util.argToAddr(masterAddrS).get;
          val clientAddr = Util.argToAddr(clientAddrS).get;
          BenchmarkClient.run(clientAddr.addr, masterAddr.addr, masterAddr.port);
          logger.debug(s"Finished client $clientAddrS");
        }
      };
      clientThread.start();
      clientThread
    };

    val runnerAddr = Util.argToAddr(runnerAddrS).get;
    var benchChannel = ManagedChannelBuilder.forAddress(runnerAddr.addr, runnerAddr.port).usePlaintext().build();
    var benchStub = BenchmarkRunnerGrpc.stub(benchChannel);
    try {

      var attempts = 0;
      var ready = false;
      while (!ready && attempts < 20) {
        attempts += 1;
        try {
          logger.info(s"Checking if runner is ready, attempt #${attempts}");
          val readyF = benchStub.ready(ReadyRequest());
          val res = Await.result(readyF, 500.milliseconds);
          if (res.status) {
            logger.info("Runner is ready!");
            ready = true
          } else {
            logger.info("Runner wasn't ready, yet.");
            Thread.sleep(500);
          }
        } catch {
          case e: Throwable => {
            logger.error("Couldn't connect to runner server. Redoing setup and retrying...", e);
            TestUtil.shutdownChannel(benchChannel);
            benchChannel = ManagedChannelBuilder.forAddress(runnerAddr.addr, runnerAddr.port).usePlaintext().build();
            benchStub = BenchmarkRunnerGrpc.stub(benchChannel);
            Thread.sleep(500);
          }
        }
      }
      ready should be(true);

      /*
       * Clean Up
       */
      logger.debug("Sending shutdown request to master");
      val sreq = ShutdownRequest().withForce(false);
      val shutdownResF = benchStub.shutdown(sreq);

      logger.debug("Waiting for master to finish...");
      masterThread.join();
      logger.debug("Master is done.");
      logger.debug("Waiting for all clients to finish...");
      clientThreads.foreach(t => t.join());
      logger.debug("All clients are done.");
    } catch {
      case e: org.scalatest.exceptions.TestFailedException => throw e // let these pass through
      case e: Throwable => {
        logger.error("Error during test", e);
        Console.err.println("Thrown error:");
        e.printStackTrace(Console.err);
        Console.err.println("Caused by:");
        e.getCause().printStackTrace(Console.err);
        fail(e);
      }
    } finally {
      benchStub = null;
      if (benchChannel != null) {
        TestUtil.shutdownChannel(benchChannel);
      }
    }

    logger.info(s"""
%%%%%%%%%%%%%%%%%%%%%%%%%%%
%% MASTER-CLIENT SUMMARY %%
%%%%%%%%%%%%%%%%%%%%%%%%%%%
${implemented.size} tests implemented: ${implemented.mkString(",")}
${notImplemented.size} tests not implemented: ${notImplemented.mkString(",")}
""")
  }

  private def checkResult(label: String, trf: Future[TestResult]): Unit = {
    val trfReady = Await.ready(trf, timeout);
    trfReady.value.get match {
      case Success(tr) => {
        tr match {
          case s: TestSuccess => {
            s.runResults.size should equal(s.numberOfRuns);
            s.runResults.size should be >= BenchmarkRunner.MIN_RUNS;
            implemented ::= label;
          }
          case f: TestFailure => {
            this.allowedFailures match {
              case rseErr :: Nil => {
                f.reason should include(rseErr); // since tests are short they may not meet RSE requirements
              }
              case testErr :: rseErr :: Nil => {
                f.reason should (include(testErr) or include(rseErr));
              }
              case _ => {
                ???
              }
            }
            implemented ::= label;
          }
          case n: NotImplemented => {
            logger.warn(s"Test $label was not implemented");
            notImplemented ::= label;
          }
          case x => fail(s"Unexpected test result: $x")
        }
      }
      case Failure(e) => {
        logger.error(s"Test $label failed in promise/future!", e);
        e.printStackTrace(Console.err);
        fail(s"Test failed due to gRPC communication: ${e.getMessage()}")
      }
    }
  }
}
