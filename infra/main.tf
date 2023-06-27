terraform {
  required_providers {
    kubernetes = {
      source = "hashicorp/kubernetes"
    }
  }
  backend "kubernetes" {
    namespace     = "terraform-backend"
    secret_suffix = "nordic-wellness-booker"
    config_path   = "~/.kube/config"
  }
}

provider "kubernetes" {
  config_path = "~/.kube/config"
}

locals {
  namespace = "nordic-wellness-booker"
  name      = "nordic-wellness-booker"
}

data "terraform_remote_state" "rsb_config" {
  backend = "kubernetes"
  config = {
    namespace     = "terraform-backend"
    secret_suffix = "rsb-config"
    config_path   = "~/.kube/config"
  }
}

variable "image_tag" {
  type = string
}

variable "rsb_config_api_key" {
  type = string
}

resource "kubernetes_namespace_v1" "nordic_wellness_booker_ns" {
  metadata {
    name = local.namespace
  }
}


resource "kubernetes_secret_v1" "env" {
  metadata {
    name      = local.name
    namespace = local.namespace
  }
  data = {
    RSB_CONFIG_URL     = data.terraform_remote_state.rsb_config.outputs.rsb_config_url
    RSB_CONFIG_API_KEY = var.rsb_config_api_key
  }
}

resource "kubernetes_deployment_v1" "nordic_wellness_booker_deploy" {
  metadata {
    name      = local.name
    namespace = local.namespace
  }

  spec {
    replicas = 1
    selector {
      match_labels = {
        app = local.name
      }
    }

    template {
      metadata {
        labels = {
          app = local.name
        }
      }
      spec {
        container {
          name  = local.name
          image = "maxrsb/nordic_wellness_booker:${var.image_tag}"
          env_from {
            secret_ref {
              name = kubernetes_secret_v1.env.metadata[0].name
            }
          }
          resources {
            requests = {
              cpu    = "20m"
              memory = "5Mi"
            }

            limits = {
              cpu    = "100m"
              memory = "20Mi"
            }
          }
        }
      }
    }
  }
}
